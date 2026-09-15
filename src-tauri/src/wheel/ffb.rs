//! Userspace force-feedback driver for classic Logitech wheels.
//!
//! Games talk force feedback through the Linux input API: they upload
//! `ff_effect`s to an event device and start/stop them. Only a driver can be
//! on the receiving end of that — normally a kernel module. Here it is a
//! uinput device instead: OpenGHub creates a virtual wheel with FF
//! capability, mirrors the real wheel's axes and buttons onto it, and turns
//! the uploaded effects into the wheel's four force slots:
//!
//! - slot 0: the sum of constant, ramp and periodic effects (a signed level),
//! - slot 1: spring, slot 2: damper, slot 3: friction (condition effects).
//!
//! The maths is a port of `new-lg4ff`'s effect engine (envelopes, direction
//! gain, periodic phase, slot command encoding), run on a 2 ms tick. The
//! real wheel's evdev node is grabbed while the bridge runs so games see one
//! wheel, not two.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use hidapi::HidDevice;
use parking_lot::Mutex;

use crate::uinput::{self as ui, Effect, EffectKind, VirtualDevice};
use super::{send_command, WheelModel, WheelSettings, WheelState};
use crate::hidpp::{Error, Result};

const MAX_EFFECTS: usize = 16;
const TICK: Duration = Duration::from_millis(2);
/// How often the wheel's inputs are forwarded even when idle (Hz).
const INPUT_POLL_HZ: u64 = 250;

// --- scaling helpers, as in new-lg4ff ------------------------------------------

fn clamp_u16(x: i64) -> u16 {
    x.clamp(0, 0xffff) as u16
}
fn clamp_s16(x: i64) -> i16 {
    x.clamp(-0x8000, 0x7fff) as i16
}
fn scale_u16(x: i64, bits: u32) -> u8 {
    (clamp_u16(x) >> (16 - bits)) as u8
}
fn scale_coeff(x: i64, bits: u32) -> u8 {
    scale_u16(x.abs() * 2, bits)
}
fn translate_force(x: i64) -> u8 {
    ((clamp_s16(x) as i32 + 0x8000) >> 8) as u8
}
/// sin(degrees) in 1.15 fixed point.
fn sin16(deg: i64) -> i64 {
    ((deg as f64).to_radians().sin() * 0x7fff as f64).round() as i64
}

// --- slots ------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotKind {
    Constant,
    Spring,
    Damper,
    Friction,
}

#[derive(Debug, Default, Clone, Copy)]
struct SlotParams {
    level: i64,
    d1: i64,
    d2: i64,
    k1: i64,
    k2: i64,
    clip: i64,
}

struct Slot {
    id: u8,
    kind: SlotKind,
    cmd_op: u8,
    cmd: [u8; 7],
    dirty: bool,
}

impl Slot {
    fn new(id: u8, kind: SlotKind) -> Self {
        Slot { id, kind, cmd_op: 0, cmd: [0; 7], dirty: false }
    }

    /// `lg4ff_update_slot`: re-encodes the slot from its parameters and marks
    /// it dirty when the bytes changed.
    fn update(&mut self, p: &SlotParams) {
        let mut original = self.cmd;
        if original[0] & 0x0f == 1 {
            original[0] = (original[0] & 0xf0) + 0x0c;
        }

        if self.kind == SlotKind::Constant {
            self.cmd_op = if self.cmd_op == 0 { 1 } else { 0x0c };
        } else if p.clip == 0 {
            self.cmd_op = 3;
        } else if self.cmd_op == 3 {
            self.cmd_op = 1;
        } else {
            self.cmd_op = 0x0c;
        }

        let mut cmd = [0u8; 7];
        cmd[0] = (0x10 << self.id) + self.cmd_op;
        if self.cmd_op != 3 {
            match self.kind {
                SlotKind::Constant => {
                    cmd[2 + self.id as usize] = translate_force(p.level);
                }
                SlotKind::Spring => {
                    let mut d1 = scale_u16((p.d1 + 0x8000) & 0xffff, 11) as i64;
                    let mut d2 = scale_u16((p.d2 + 0x8000) & 0xffff, 11) as i64;
                    let s1 = (p.k1 < 0) as u8;
                    let s2 = (p.k2 < 0) as u8;
                    let mut k1 = p.k1.abs();
                    let mut k2 = p.k2.abs();
                    if k1 < 2048 {
                        d1 = 0;
                    } else {
                        k1 -= 2048;
                    }
                    if k2 < 2048 {
                        d2 = 2047;
                    } else {
                        k2 -= 2048;
                    }
                    cmd[1] = 0x0b;
                    cmd[2] = (d1 >> 3) as u8;
                    cmd[3] = (d2 >> 3) as u8;
                    cmd[4] = (scale_coeff(k2, 4) << 4) + scale_coeff(k1, 4);
                    cmd[5] = (((d2 & 7) << 5) + ((d1 & 7) << 1)) as u8 + (s2 << 4) + s1;
                    cmd[6] = scale_u16(p.clip, 8);
                }
                SlotKind::Damper => {
                    cmd[1] = 0x0c;
                    cmd[2] = scale_coeff(p.k1, 4);
                    cmd[3] = (p.k1 < 0) as u8;
                    cmd[4] = scale_coeff(p.k2, 4);
                    cmd[5] = (p.k2 < 0) as u8;
                    cmd[6] = scale_u16(p.clip, 8);
                }
                SlotKind::Friction => {
                    cmd[1] = 0x0e;
                    cmd[2] = scale_coeff(p.k1, 8);
                    cmd[3] = scale_coeff(p.k2, 8);
                    cmd[4] = scale_u16(p.clip, 8);
                    cmd[5] = ((p.k2 < 0) as u8) << 4 | (p.k1 < 0) as u8;
                }
            }
        }
        self.cmd = cmd;
        if original != cmd {
            self.dirty = true;
        }
    }
}

// --- effect states ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
struct EffectState {
    effect: Option<Effect>,
    started: bool,
    all_set: bool,
    playing: bool,
    updating: bool,
    count: i32,
    start_at: u64,
    play_at: u64,
    stop_at: u64,
    updated_at: u64,
    time_playing: u64,
    direction_gain: i64,
    phase: i64,
    phase_adj: i64,
    slope: i64,
}

fn envelope_of(e: &Effect) -> ui::Envelope {
    match e.kind {
        EffectKind::Constant { envelope, .. } | EffectKind::Ramp { envelope, .. } | EffectKind::Periodic { envelope, .. } => envelope,
        _ => ui::Envelope::default(),
    }
}

impl EffectState {
    fn stop(&mut self) {
        self.started = false;
        self.all_set = false;
        self.playing = false;
    }

    /// `lg4ff_update_state`.
    fn update(&mut self, now: u64) {
        let Some(effect) = self.effect else { return };
        let env = envelope_of(&effect);
        if !self.all_set {
            self.all_set = true;
            self.play_at = self.start_at + effect.replay_delay as u64;
            if !self.updating {
                self.updated_at = self.play_at;
            }
            self.direction_gain = sin16(effect.direction as i64 * 360 / 0x10000);
            if let EffectKind::Periodic { phase, period, .. } = effect.kind {
                self.phase_adj = if period > 0 { phase as i64 * 360 / period as i64 } else { 0 };
            }
            if effect.replay_length > 0 {
                self.stop_at = self.play_at + effect.replay_length as u64;
            }
        }
        if self.updating {
            self.updating = false;
            self.playing = false;
            self.play_at = self.updated_at + effect.replay_delay as u64;
            self.direction_gain = sin16(effect.direction as i64 * 360 / 0x10000);
            if effect.replay_length > 0 {
                self.stop_at = self.updated_at + effect.replay_length as u64;
            }
            if matches!(effect.kind, EffectKind::Periodic { .. }) {
                self.phase_adj = self.phase;
            }
        }

        self.slope = 0;
        if let EffectKind::Ramp { start, end, .. } = effect.kind {
            if effect.replay_length > 0 {
                let span = effect.replay_length as i64 - env.attack_length as i64 - env.fade_length as i64;
                if span > 0 {
                    self.slope = ((end as i64 - start as i64) << 16) / span;
                }
            }
        }

        if !self.playing && now >= self.play_at && (effect.replay_length == 0 || now < self.stop_at) {
            self.playing = true;
        }
        if self.playing {
            self.time_playing = now.saturating_sub(self.play_at);
            if let EffectKind::Periodic { period, .. } = effect.kind {
                if period > 0 {
                    let phase_time = now.saturating_sub(self.updated_at);
                    self.phase = (phase_time % period as u64) as i64 * 360 / period as i64;
                    self.phase += self.phase_adj % 360;
                }
            }
        }
    }

    fn constant_level(&self, level: i64, env: &ui::Envelope, replay_length: u64) -> i64 {
        let mut level = level;
        let t = self.time_playing as i64;
        if (t as u64) < env.attack_length as u64 {
            let sign = if level < 0 { -1 } else { 1 };
            let d = level - sign * env.attack_level as i64;
            level = sign * env.attack_level as i64 + d * t / env.attack_length as i64;
        } else if replay_length > 0 {
            let tt = t - replay_length as i64 + env.fade_length as i64;
            if tt > 0 && env.fade_length > 0 {
                let sign = if level < 0 { -1 } else { 1 };
                let d = level - sign * env.fade_level as i64;
                level -= d * tt / env.fade_length as i64;
            }
        }
        self.direction_gain * level / 0x7fff
    }

    fn ramp_level(&self, start: i64, end: i64, env: &ui::Envelope, replay_length: u64) -> i64 {
        let t = self.time_playing as i64;
        let level;
        if (t as u64) < env.attack_length as u64 {
            let sign = if start < 0 { -1 } else { 1 };
            let tt = env.attack_length as i64 - t;
            let d = start - sign * env.attack_level as i64;
            level = sign * env.attack_level as i64 + d * tt / env.attack_length as i64;
        } else if replay_length > 0 && t >= replay_length as i64 - env.fade_length as i64 && env.fade_length > 0 {
            let sign = if end < 0 { -1 } else { 1 };
            let tt = t - replay_length as i64 + env.fade_length as i64;
            let d = sign * env.fade_level as i64 - end;
            level = end - d * tt / env.fade_length as i64;
        } else {
            let tt = t - env.attack_length as i64;
            level = start + ((tt * self.slope) >> 16);
        }
        self.direction_gain * level / 0x7fff
    }

    fn periodic_level(&self, waveform: u16, magnitude: i64, offset: i64, env: &ui::Envelope, replay_length: u64) -> i64 {
        let mut magnitude = magnitude;
        let sign = if magnitude < 0 { -1 } else { 1 };
        let t = self.time_playing as i64;
        if (t as u64) < env.attack_length as u64 {
            let d = magnitude - sign * env.attack_level as i64;
            magnitude = sign * env.attack_level as i64 + d * t / env.attack_length as i64;
        } else if replay_length > 0 {
            let tt = t - replay_length as i64 + env.fade_length as i64;
            if tt > 0 && env.fade_length > 0 {
                let d = magnitude - sign * env.fade_level as i64;
                magnitude -= d * tt / env.fade_length as i64;
            }
        }
        let phase = self.phase;
        let level = offset
            + match waveform {
                ui::FF_SINE => sin16(phase) * magnitude / 0x7fff,
                ui::FF_SQUARE => (if phase < 180 { 1 } else { -1 }) * magnitude,
                ui::FF_TRIANGLE => (phase * magnitude * 2 / 360 - magnitude).abs() * 2 - magnitude,
                ui::FF_SAW_UP => phase * magnitude * 2 / 360 - magnitude,
                ui::FF_SAW_DOWN => magnitude - phase * magnitude * 2 / 360,
                _ => 0,
            };
        self.direction_gain * level / 0x7fff
    }
}

// --- the engine -------------------------------------------------------------------

/// Everything the tick needs, kept behind one lock shared with the event
/// handlers.
struct Engine {
    states: [EffectState; MAX_EFFECTS],
    effects_used: usize,
    slots: [Slot; 4],
    /// FF_GAIN from the game, 0..0xffff.
    game_gain: i64,
    /// FF_AUTOCENTER from the game, 0..0xffff; `None` = game never set it.
    game_autocenter: Option<u16>,
    /// What the spring command currently says, to avoid re-sending.
    spring_sent: Option<u8>,
    epoch: Instant,
}

impl Engine {
    fn new() -> Self {
        Engine {
            states: [EffectState::default(); MAX_EFFECTS],
            effects_used: 0,
            slots: [
                Slot::new(0, SlotKind::Constant),
                Slot::new(1, SlotKind::Spring),
                Slot::new(2, SlotKind::Damper),
                Slot::new(3, SlotKind::Friction),
            ],
            game_gain: 0xffff,
            game_autocenter: None,
            spring_sent: None,
            epoch: Instant::now(),
        }
    }

    fn now_ms(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64
    }

    fn upload(&mut self, effect: Effect) -> std::result::Result<i16, i32> {
        // Re-upload of an existing id updates it in place.
        let id = if effect.id >= 0 && (effect.id as usize) < MAX_EFFECTS {
            effect.id as usize
        } else {
            match self.states.iter().position(|s| s.effect.is_none()) {
                Some(i) => i,
                None => return Err(-libc::ENOSPC),
            }
        };
        let st = &mut self.states[id];
        let was_started = st.started;
        st.effect = Some(Effect { id: id as i16, ..effect });
        if was_started {
            st.updating = true;
            st.updated_at = self.epoch.elapsed().as_millis() as u64;
        }
        Ok(id as i16)
    }

    fn erase(&mut self, id: u32) {
        if let Some(st) = self.states.get_mut(id as usize) {
            if st.started {
                self.effects_used = self.effects_used.saturating_sub(1);
            }
            *st = EffectState::default();
        }
    }

    fn play(&mut self, id: usize, count: i32) {
        let now = self.now_ms();
        let Some(st) = self.states.get_mut(id) else { return };
        if st.effect.is_none() {
            return;
        }
        if count > 0 {
            if !st.started {
                self.effects_used += 1;
            }
            st.started = true;
            st.all_set = false;
            st.playing = false;
            st.count = count;
            st.start_at = now;
        } else if st.started {
            st.stop();
            self.effects_used = self.effects_used.saturating_sub(1);
        }
    }

    fn any_playing(&self) -> bool {
        self.effects_used > 0
    }

    /// One tick: evaluates every effect into the four slots and returns the
    /// slot commands whose bytes changed.
    fn tick(&mut self, master_gain: i64) -> Vec<[u8; 7]> {
        let now = self.now_ms();
        let mut params = [SlotParams::default(); 4];
        let gain = master_gain * self.game_gain / 0xffff;

        let mut remaining = self.effects_used;
        for st in self.states.iter_mut() {
            if remaining == 0 {
                break;
            }
            if !st.started {
                continue;
            }
            remaining -= 1;
            let Some(effect) = st.effect else { continue };
            if st.all_set && effect.replay_length > 0 && now >= st.stop_at {
                st.stop();
                st.count -= 1;
                if st.count <= 0 {
                    self.effects_used = self.effects_used.saturating_sub(1);
                    continue;
                }
                st.started = true;
                st.start_at = st.stop_at;
            }
            st.update(now);
            if !st.playing {
                continue;
            }
            let env = envelope_of(&effect);
            let len = effect.replay_length as u64;
            match effect.kind {
                EffectKind::Constant { level, .. } => params[0].level += st.constant_level(level as i64, &env, len),
                EffectKind::Ramp { start, end, .. } => params[0].level += st.ramp_level(start as i64, end as i64, &env, len),
                EffectKind::Periodic { waveform, magnitude, offset, .. } => {
                    params[0].level += st.periodic_level(waveform, magnitude as i64, offset as i64, &env, len)
                }
                EffectKind::Condition { type_, condition: c } => {
                    let slot = match type_ {
                        ui::FF_SPRING => 1,
                        ui::FF_DAMPER | ui::FF_INERTIA => 2,
                        ui::FF_FRICTION => 3,
                        _ => continue,
                    };
                    if slot == 1 {
                        params[1].d1 = c.center as i64 - c.deadband as i64 / 2;
                        params[1].d2 = c.center as i64 + c.deadband as i64 / 2;
                    }
                    params[slot].k1 = c.left_coeff as i64;
                    params[slot].k2 = c.right_coeff as i64;
                    params[slot].clip = c.right_saturation as i64;
                }
                EffectKind::Unsupported(_) => {}
            }
        }

        params[0].level = params[0].level * gain / 0xffff;
        for p in params.iter_mut().skip(1) {
            p.k1 = p.k1 * gain / 0xffff;
            p.k2 = p.k2 * gain / 0xffff;
            p.clip = p.clip * gain / 0xffff;
        }

        let mut out = Vec::new();
        for (slot, p) in self.slots.iter_mut().zip(params.iter()) {
            slot.update(p);
            if slot.dirty {
                out.push(slot.cmd);
                slot.dirty = false;
            }
        }
        out
    }

    /// Slot 0..3 reset commands, sent when the bridge starts and stops.
    fn reset_commands(&mut self) -> Vec<[u8; 7]> {
        let mut out = Vec::new();
        for slot in self.slots.iter_mut() {
            slot.cmd_op = 0;
            slot.cmd = [0; 7];
            slot.update(&SlotParams::default());
            out.push(slot.cmd);
            slot.dirty = false;
        }
        out
    }
}

// --- the bridge thread ------------------------------------------------------------

/// Shared knobs the UI can change while the bridge runs.
#[derive(Default)]
struct Shared {
    settings: Mutex<WheelSettings>,
}

pub struct Bridge {
    stop: Arc<AtomicBool>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Bridge {
    pub fn start(
        model: &'static WheelModel,
        device: Arc<Mutex<HidDevice>>,
        state: Arc<Mutex<WheelState>>,
        settings: Arc<Mutex<WheelSettings>>,
        grab_paths: Vec<String>,
    ) -> Result<Self> {
        let axes = [
            ui::Axis { code: ui::ABS_X, min: 0, max: 65535, fuzz: 0, flat: 0 },
            ui::Axis { code: ui::ABS_Z, min: 0, max: 65535, fuzz: 0, flat: 0 },
            ui::Axis { code: ui::ABS_RZ, min: 0, max: 65535, fuzz: 0, flat: 0 },
            ui::Axis { code: ui::ABS_Y, min: 0, max: 65535, fuzz: 0, flat: 0 },
            ui::Axis { code: ui::ABS_HAT0X, min: -1, max: 1, fuzz: 0, flat: 0 },
            ui::Axis { code: ui::ABS_HAT0Y, min: -1, max: 1, fuzz: 0, flat: 0 },
        ];
        let buttons: Vec<u16> = (0..16u16).map(|i| ui::BTN_TRIGGER + i).chain((0..12u16).map(|i| ui::BTN_TRIGGER_HAPPY + i)).collect();
        // Present the native product id so games with per-wheel support
        // (SDL mappings, sims) recognise it.
        let pid = if model.product_id == 0xc267 { 0xc266 } else { model.product_id };
        let vdev = VirtualDevice::create(
            &format!("OpenGHub {}", model.name),
            crate::hidpp::LOGITECH_VID,
            pid,
            &axes,
            &buttons,
            MAX_EFFECTS as u32,
        )
        .map_err(|e| Error::other(format!("cannot create the virtual wheel on /dev/uinput: {e}")))?;

        let grabs: Vec<ui::Grab> = grab_paths
            .iter()
            .filter_map(|p| match ui::Grab::take(p) {
                Ok(g) => Some(g),
                Err(e) => {
                    log::warn!("could not grab {p}: {e}");
                    None
                }
            })
            .collect();

        let shared = Arc::new(Shared { settings: Mutex::new(settings.lock().clone()) });
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let stop = Arc::clone(&stop);
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("wheel-ffb".into())
                .spawn(move || run(model, device, state, settings, vdev, grabs, shared, stop))
                .map_err(|e| Error::other(e.to_string()))?
        };
        Ok(Bridge { stop, shared, thread: Some(thread) })
    }

    pub fn update_settings(&self, settings: &WheelSettings) {
        *self.shared.settings.lock() = settings.clone();
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run(
    model: &'static WheelModel,
    device: Arc<Mutex<HidDevice>>,
    state: Arc<Mutex<WheelState>>,
    settings: Arc<Mutex<WheelSettings>>,
    vdev: VirtualDevice,
    _grabs: Vec<ui::Grab>,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
) {
    let mut engine = Engine::new();
    let send = |cmd: [u8; 7]| {
        if let Err(e) = send_command(&device.lock(), model.protocol, cmd) {
            log::warn!("wheel command failed: {e}");
        }
    };
    for cmd in engine.reset_commands() {
        send(cmd);
    }

    let mut last_forward = Instant::now();
    let mut spring_state: Option<u8> = None;

    while !stop.load(Ordering::SeqCst) {
        let tick_start = Instant::now();

        // 1. Requests from games.
        match vdev.read_events() {
            Ok(events) => {
                for ev in events {
                    match ev.type_ {
                        ui::EV_UINPUT if ev.code == ui::UI_FF_UPLOAD => {
                            let r = vdev.handle_upload(ev.value, |raw| engine.upload(Effect::from_raw(raw)));
                            if let Err(e) = r {
                                log::warn!("ff upload failed: {e}");
                            }
                        }
                        ui::EV_UINPUT if ev.code == ui::UI_FF_ERASE => {
                            let r = vdev.handle_erase(ev.value, |id| engine.erase(id));
                            if let Err(e) = r {
                                log::warn!("ff erase failed: {e}");
                            }
                        }
                        ui::EV_FF => match ev.code {
                            ui::FF_GAIN => engine.game_gain = ev.value.clamp(0, 0xffff) as i64,
                            ui::FF_AUTOCENTER => engine.game_autocenter = Some(ev.value.clamp(0, 0xffff) as u16),
                            id => engine.play(id as usize, ev.value),
                        },
                        _ => {}
                    }
                }
            }
            Err(e) => log::warn!("uinput read failed: {e}"),
        }

        // 2. Forces.
        let cfg = shared.settings.lock().clone();
        let master = cfg.ffb_gain.min(100) as i64 * 0xffff / 100;
        for cmd in engine.tick(master) {
            send(cmd);
        }

        // 3. Centering spring: the game's autocenter wins while it plays
        //    effects, otherwise the profile's spring — unless the user wants
        //    the spring under force feedback too.
        let want: u8 = match engine.game_autocenter {
            Some(v) if engine.any_playing() || v > 0 => (v as u32 * 100 / 0xffff) as u8,
            _ if engine.any_playing() && !cfg.center_spring_in_ffb_games => 0,
            _ => cfg.center_spring,
        };
        if spring_state != Some(want) {
            spring_state = Some(want);
            let cmds = spring_commands(want);
            for c in cmds {
                send(c);
            }
        }
        engine.spring_sent = spring_state;

        // 4. Inputs → virtual wheel.
        {
            let dev = device.lock();
            let mut buf = [0u8; 64];
            let mut latest = None;
            loop {
                let n = dev.read_timeout(&mut buf, 0).unwrap_or(0);
                if n == 0 {
                    break;
                }
                let parsed = match model.protocol {
                    super::Protocol::ClassicReport30 => super::parse_ps_report(&buf[..n]),
                    _ => super::parse_native_report(&buf[..n]),
                };
                if let Some(s) = parsed {
                    latest = Some(s);
                }
            }
            drop(dev);
            if let Some(raw) = latest {
                let s = super::apply_settings(raw, &settings.lock());
                *state.lock() = s;
                forward(&vdev, &s);
                last_forward = Instant::now();
            } else if last_forward.elapsed() > Duration::from_millis(1000 / INPUT_POLL_HZ) {
                // Keep the virtual device "alive" for readers that time out.
                last_forward = Instant::now();
            }
        }

        let elapsed = tick_start.elapsed();
        if elapsed < TICK {
            std::thread::sleep(TICK - elapsed);
        }
    }

    for cmd in engine.reset_commands() {
        send(cmd);
    }
    let _ = vdev;
}

/// The classic spring command pair for a 0-100 strength (0 = off).
fn spring_commands(percent: u8) -> Vec<[u8; 7]> {
    if percent == 0 {
        return vec![[0xf5, 0, 0, 0, 0, 0, 0]];
    }
    let magnitude = (percent.min(100) as u32 * 0xffff) / 100;
    let (a, b) = if magnitude <= 0xaaaa {
        (0x0c * magnitude, 0x80 * magnitude)
    } else {
        (0x0c * 0xaaaa + 0x06 * (magnitude - 0xaaaa), 0x80 * 0xaaaa + 0xff * (magnitude - 0xaaaa))
    };
    let a = a >> 1;
    vec![
        [0xfe, 0x0d, (a / 0xaaaa) as u8, (a / 0xaaaa) as u8, (b / 0xaaaa) as u8, 0, 0],
        [0x14, 0, 0, 0, 0, 0, 0],
    ]
}

/// Mirrors a state onto the virtual device. Pedals keep Logitech's raw sense
/// (max = released), as the kernel driver reports them.
fn forward(vdev: &VirtualDevice, s: &WheelState) {
    let steering = (((s.steering + 1.0) / 2.0).clamp(0.0, 1.0) * 65535.0) as i32;
    let pedal = |v: f32| ((1.0 - v).clamp(0.0, 1.0) * 65535.0) as i32;
    let (hx, hy) = match s.hat {
        0 => (0, -1),
        1 => (1, -1),
        2 => (1, 0),
        3 => (1, 1),
        4 => (0, 1),
        5 => (-1, 1),
        6 => (-1, 0),
        7 => (-1, -1),
        _ => (0, 0),
    };
    let mut events = vec![
        (ui::EV_ABS, ui::ABS_X, steering),
        (ui::EV_ABS, ui::ABS_Z, pedal(s.accelerator)),
        (ui::EV_ABS, ui::ABS_RZ, pedal(s.brake)),
        (ui::EV_ABS, ui::ABS_Y, pedal(s.clutch)),
        (ui::EV_ABS, ui::ABS_HAT0X, hx),
        (ui::EV_ABS, ui::ABS_HAT0Y, hy),
    ];
    for i in 0..28u16 {
        let code = if i < 16 { ui::BTN_TRIGGER + i } else { ui::BTN_TRIGGER_HAPPY + (i - 16) };
        events.push((ui::EV_KEY, code, ((s.buttons >> i) & 1) as i32));
    }
    if let Err(e) = vdev.emit(&events) {
        log::debug!("uinput emit failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_slot_encodes_like_lg4ff() {
        let mut slot = Slot::new(0, SlotKind::Constant);
        slot.update(&SlotParams { level: 0, ..Default::default() });
        // First update: op 1, level 0 → 0x80.
        assert_eq!(slot.cmd, [0x11, 0x00, 0x80, 0, 0, 0, 0]);
        slot.dirty = false;
        slot.update(&SlotParams { level: 0x7fff, ..Default::default() });
        assert_eq!(slot.cmd[0], 0x1c);
        assert_eq!(slot.cmd[2], 0xff);
        assert!(slot.dirty);
    }

    #[test]
    fn empty_condition_slots_stop() {
        let mut slot = Slot::new(1, SlotKind::Spring);
        slot.update(&SlotParams::default());
        assert_eq!(slot.cmd, [0x23, 0, 0, 0, 0, 0, 0]);
        slot.update(&SlotParams { k1: 0x4000, k2: 0x4000, clip: 0xffff, ..Default::default() });
        assert_eq!(slot.cmd[0], 0x21);
        assert_eq!(slot.cmd[1], 0x0b);
        assert_eq!(slot.cmd[6], 0xff);
    }

    #[test]
    fn spring_commands_match_the_handle() {
        assert_eq!(spring_commands(0), vec![[0xf5, 0, 0, 0, 0, 0, 0]]);
        let full = spring_commands(100);
        assert_eq!(full[0][0], 0xfe);
        assert_eq!(full[0][1], 0x0d);
        assert_eq!(full[1][0], 0x14);
    }

    #[test]
    fn constant_effect_plays_and_expires() {
        let mut engine = Engine::new();
        let effect = Effect {
            id: -1,
            direction: 0x4000,
            replay_length: 5,
            replay_delay: 0,
            kind: EffectKind::Constant { level: 0x4000, envelope: ui::Envelope::default() },
        };
        let id = engine.upload(effect).unwrap();
        engine.play(id as usize, 1);
        let cmds = engine.tick(0xffff);
        assert!(!cmds.is_empty(), "first tick emits the constant slot");
        assert!(cmds[0][2] > 0x80, "positive force pushes above centre");
        std::thread::sleep(Duration::from_millis(8));
        engine.tick(0xffff);
        assert_eq!(engine.effects_used, 0, "effect expires after its replay length");
    }
}
