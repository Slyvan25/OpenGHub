//! Keyboard animations (G HUB's LIGHTSYNC ANIMATIONS): a sequence of
//! Freestyle frames, played by OpenGHub by writing per-key colours — and
//! Colour Drift, a generated one: every key fades to a random hue of its
//! own, pauses, and picks the next.
//!
//! Each frame is shown for its duration; a "fade" transition blends into the
//! next one in a few steps. Only keys whose colour changed are written after
//! the first frame, which keeps a wireless keyboard responsive.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use tauri::Manager;

use crate::state::DeviceManager;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    /// LED id (as a string key) → `#rrggbb`; missing keys are off.
    #[serde(default)]
    pub keys: HashMap<String, String>,
    /// How long this frame shows, in ms.
    #[serde(default = "default_frame_ms")]
    pub duration_ms: u32,
}

fn default_frame_ms() -> u32 {
    300
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Animation {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub frames: Vec<Frame>,
    /// `cycle` | `reverse` | `bounce` | `random`
    #[serde(default = "default_cycle")]
    pub cycle: String,
    /// `none` | `fade`
    #[serde(default = "default_transition")]
    pub transition: String,
}

fn default_cycle() -> String {
    "cycle".into()
}
fn default_transition() -> String {
    "fade".into()
}

/// Colour Drift: each key wanders the colour wheel on its own.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Drift {
    /// LED ids to drive — the keyboard's key map, supplied by the UI.
    #[serde(default)]
    pub keys: Vec<u8>,
    /// Length of each fade to the next set of hues, ms.
    #[serde(default = "default_fade_ms")]
    pub fade_ms: u32,
    /// Pause on each set before the next fade starts, ms.
    #[serde(default = "default_hold_ms")]
    pub hold_ms: u32,
    /// 0–100; 100 is pure hues, lower mixes in white.
    #[serde(default = "full")]
    pub saturation: u8,
    /// 0–100.
    #[serde(default = "full")]
    pub brightness: u8,
}

fn default_fade_ms() -> u32 {
    1000
}
fn default_hold_ms() -> u32 {
    300
}
fn full() -> u8 {
    100
}

/// Hue 0–1 (wrapping), saturation and value 0–1, to RGB.
pub fn hsv(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i as u32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    let byte = |x: f32| (x * 255.0).round().clamp(0.0, 255.0) as u8;
    [byte(r), byte(g), byte(b)]
}

/// xorshift64*, seeded from the clock: good enough for picking hues, with no
/// RNG dependency.
struct Rng(u64);

impl Rng {
    fn seeded() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);
        Rng(nanos | 1)
    }
    /// Uniform in [0, 1).
    fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// The shortest way around the colour wheel from `from` to `to`, in -0.5..0.5.
pub fn hue_delta(from: f32, to: f32) -> f32 {
    (to - from + 0.5).rem_euclid(1.0) - 0.5
}

type Colours = HashMap<u8, [u8; 3]>;

/// Writes frames to a keyboard, sending only what changed since the last one.
struct FrameWriter {
    device: String,
    shown: Option<Colours>,
}

impl FrameWriter {
    fn show(&mut self, manager: &DeviceManager, next: Colours) {
        let first = self.shown.is_none();
        let mut diff: Vec<(u8, [u8; 3])> = next
            .iter()
            .filter(|(led, c)| self.shown.as_ref().and_then(|s| s.get(led)) != Some(c))
            .map(|(l, c)| (*l, *c))
            .collect();
        // Keys lit before and absent now go dark.
        if let Some(prev) = &self.shown {
            diff.extend(prev.keys().filter(|l| !next.contains_key(l)).map(|l| (*l, [0; 3])));
        }
        if first || !diff.is_empty() {
            if let Err(e) = manager.set_per_key_frame(&self.device, &diff, first) {
                log::debug!("animation frame not written: {e}");
            }
        }
        self.shown = Some(next);
    }
}

fn colours(frame: &Frame) -> Colours {
    frame
        .keys
        .iter()
        .filter_map(|(led, hex)| Some((led.parse().ok()?, crate::lightsync::parse_hex(hex))))
        .collect()
}

/// Frame indices in play order, for one pass.
fn order(a: &Animation, pass: u64) -> Vec<usize> {
    let n = a.frames.len();
    match a.cycle.as_str() {
        "reverse" => (0..n).rev().collect(),
        "bounce" if n > 2 => (0..n).chain((1..n - 1).rev()).collect(),
        "random" => {
            // A cheap, deterministic shuffle per pass; no RNG dependency.
            let mut v: Vec<usize> = (0..n).collect();
            let mut x = pass.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
            for i in (1..n).rev() {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                v.swap(i, (x % (i as u64 + 1)) as usize);
            }
            v
        }
        _ => (0..n).collect(),
    }
}

fn blend(a: &Colours, b: &Colours, t: f32) -> Colours {
    let mut out = Colours::new();
    for led in a.keys().chain(b.keys()) {
        let ca = a.get(led).copied().unwrap_or([0; 3]);
        let cb = b.get(led).copied().unwrap_or([0; 3]);
        let mix = |i: usize| (ca[i] as f32 + (cb[i] as f32 - ca[i] as f32) * t).round() as u8;
        out.insert(*led, [mix(0), mix(1), mix(2)]);
    }
    out
}

/// What a device is showing.
#[derive(Debug, Clone, PartialEq)]
enum Running {
    Animation(Animation),
    Drift(Drift),
}

/// One running animation per device; starting another stops the last.
#[derive(Default)]
pub struct Animator {
    /// Bumped to stop the device's current loop.
    generation: Mutex<HashMap<String, Arc<AtomicU64>>>,
    running: Mutex<HashMap<String, Running>>,
}

impl Animator {
    /// Stops whatever runs on `device` and registers `what`; returns the
    /// liveness check the new loop polls, or `None` when it already runs.
    fn start(&self, device: &str, what: Running) -> Option<impl Fn() -> bool> {
        if self.running.lock().get(device) == Some(&what) {
            return None; // already showing exactly this
        }
        self.stop(device);
        let gen = Arc::clone(self.generation.lock().entry(device.to_string()).or_default());
        let mine = gen.load(Ordering::SeqCst);
        self.running.lock().insert(device.to_string(), what);
        Some(move || gen.load(Ordering::SeqCst) == mine)
    }

    /// Colour Drift: random hues per key, faded to and held, forever.
    pub fn play_drift(&self, app: tauri::AppHandle, device: &str, drift: Drift) {
        if drift.keys.is_empty() {
            self.stop(device);
            return;
        }
        let Some(alive) = self.start(device, Running::Drift(drift.clone())) else { return };
        let device = device.to_string();
        std::thread::Builder::new()
            .name("colour-drift".into())
            .spawn(move || {
                let manager = app.state::<DeviceManager>();
                let mut writer = FrameWriter { device, shown: None };
                let mut rng = Rng::seeded();
                let (sat, val) = (drift.saturation.min(100) as f32 / 100.0, drift.brightness.min(100) as f32 / 100.0);
                let fade = Duration::from_millis(drift.fade_ms.clamp(100, 30_000) as u64);
                let hold = Duration::from_millis(drift.hold_ms.min(60_000) as u64);
                // ~20 steps a second at most: smooth, and well within what a
                // wired keyboard takes (the G915 X is ~30 packets per step).
                let steps = (fade.as_millis() / 50).clamp(2, 60) as u32;
                let mut hue: Vec<f32> = drift.keys.iter().map(|_| rng.unit()).collect();
                while alive() {
                    let delta: Vec<f32> = hue.iter().map(|h| hue_delta(*h, rng.unit())).collect();
                    for s in 1..=steps {
                        if !alive() {
                            return;
                        }
                        let f = s as f32 / steps as f32;
                        let frame: Colours =
                            drift.keys.iter().enumerate().map(|(i, &led)| (led, hsv(hue[i] + delta[i] * f, sat, val))).collect();
                        writer.show(&manager, frame);
                        std::thread::sleep(fade / steps);
                    }
                    for (h, d) in hue.iter_mut().zip(&delta) {
                        *h = (*h + d).rem_euclid(1.0);
                    }
                    std::thread::sleep(hold);
                }
            })
            .ok();
    }

    pub fn play(&self, app: tauri::AppHandle, device: &str, anim: Animation) {
        if anim.frames.is_empty() {
            self.stop(device);
            return;
        }
        let Some(alive) = self.start(device, Running::Animation(anim.clone())) else { return };
        let device = device.to_string();
        std::thread::Builder::new()
            .name("animation".into())
            .spawn(move || {
                let manager = app.state::<DeviceManager>();
                let mut writer = FrameWriter { device, shown: None };
                let mut show = |next: Colours| writer.show(&manager, next);
                let mut pass = 0u64;
                while alive() {
                    let seq = order(&anim, pass);
                    for (i, &f) in seq.iter().enumerate() {
                        if !alive() {
                            return;
                        }
                        let frame = &anim.frames[f];
                        let here = colours(frame);
                        let hold = Duration::from_millis(frame.duration_ms.clamp(20, 60_000) as u64);
                        if anim.transition == "fade" {
                            let next = colours(&anim.frames[seq[(i + 1) % seq.len()]]);
                            // A handful of steps: smooth enough, light on the radio.
                            let steps = ((hold.as_millis() / 60) as usize).clamp(1, 8);
                            for s in 0..steps {
                                if !alive() {
                                    return;
                                }
                                show(blend(&here, &next, s as f32 / steps as f32));
                                std::thread::sleep(hold / steps as u32);
                            }
                        } else {
                            show(here);
                            std::thread::sleep(hold);
                        }
                    }
                    pass += 1;
                }
            })
            .ok();
    }

    pub fn stop(&self, device: &str) {
        if let Some(g) = self.generation.lock().get(device) {
            g.fetch_add(1, Ordering::SeqCst);
        }
        self.running.lock().remove(device);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anim(cycle: &str, n: usize) -> Animation {
        Animation {
            name: String::new(),
            frames: (0..n).map(|_| Frame { keys: HashMap::new(), duration_ms: 100 }).collect(),
            cycle: cycle.into(),
            transition: "none".into(),
        }
    }

    #[test]
    fn play_orders() {
        assert_eq!(order(&anim("cycle", 3), 0), vec![0, 1, 2]);
        assert_eq!(order(&anim("reverse", 3), 0), vec![2, 1, 0]);
        assert_eq!(order(&anim("bounce", 4), 0), vec![0, 1, 2, 3, 2, 1]);
        let mut r = order(&anim("random", 5), 7);
        r.sort();
        assert_eq!(r, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn hsv_corners_and_wrap() {
        assert_eq!(hsv(0.0, 1.0, 1.0), [255, 0, 0]);
        assert_eq!(hsv(1.0 / 3.0, 1.0, 1.0), [0, 255, 0]);
        assert_eq!(hsv(2.0 / 3.0, 1.0, 1.0), [0, 0, 255]);
        assert_eq!(hsv(1.0, 1.0, 1.0), [255, 0, 0], "hue wraps");
        assert_eq!(hsv(-1.0 / 3.0, 1.0, 1.0), [0, 0, 255], "negative hue wraps too");
        assert_eq!(hsv(0.0, 0.0, 0.5), [128, 128, 128], "no saturation is grey");
    }

    #[test]
    fn hue_takes_the_short_way_round() {
        assert!((hue_delta(0.1, 0.9) - -0.2).abs() < 1e-6);
        assert!((hue_delta(0.9, 0.1) - 0.2).abs() < 1e-6);
        assert!((hue_delta(0.2, 0.5) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn rng_is_in_range() {
        let mut r = Rng(12345);
        for _ in 0..1000 {
            let x = r.unit();
            assert!((0.0..1.0).contains(&x), "{x}");
        }
    }

    #[test]
    fn fade_halfway() {
        let a: Colours = [(1, [0, 0, 0])].into();
        let b: Colours = [(1, [200, 100, 50])].into();
        assert_eq!(blend(&a, &b, 0.5)[&1], [100, 50, 25]);
    }
}
