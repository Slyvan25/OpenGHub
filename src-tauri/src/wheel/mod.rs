//! Logitech racing wheels.
//!
//! The older wheels — and the G923 in its PlayStation mode — do not speak HID++.
//! They take the classic 7-byte command set that Logitech has used since the
//! Driving Force Pro, which G HUB drives through its raw-HID `hidio_g29` /
//! `hidio_g923_ps4` classes and the `new-lg4ff` kernel driver documents:
//!
//! | command                     | bytes                        |
//! |-----------------------------|------------------------------|
//! | operating range (degrees)   | `f8 81 lo hi 00 00 00`       |
//! | RPM LEDs (5-bit mask)       | `f8 12 mask 00 00 00 00`     |
//! | centering spring            | `fe 0d k k mag 00 00` + `14` |
//! | centering spring off        | `f5 00 00 00 00 00 00`       |
//! | switch PS mode → native     | `f8 09 07 01 01 00 00`       |
//! | force slots 0-3             | see [`ffb`]                  |
//!
//! On the G923 (PS mode) the commands travel as output report `0x30` on the
//! joystick interface; native-mode wheels use the id-less 7-byte report.
//! Verified on a G923 PS4/PC: LED chase and a 90° range, both user-confirmed.
//!
//! Force feedback for games is provided by [`ffb`]: a virtual wheel on uinput
//! that receives the effects games upload and turns them into slot commands,
//! so no kernel module is needed.

pub mod ffb;
/// The uinput binding lives at the crate root; re-exported for the bridge.
pub use crate::uinput;

use std::sync::Arc;

use hidapi::{HidApi, HidDevice};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::hidpp::{Error, Result, LOGITECH_VID};

/// How a wheel is driven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Protocol {
    /// Classic 7-byte commands, id-less output report on the joystick interface.
    Classic,
    /// Classic commands wrapped in output report `0x30` (G923 PS mode).
    ClassicReport30,
    /// HID++ 2.0 with feature `0x8123` (G920, G923 Xbox/PC). Handled by the
    /// HID++ path; listed here so the joystick interface is still recognised.
    Hidpp,
}

#[derive(Debug, Clone, Copy)]
pub struct WheelModel {
    pub product_id: u16,
    pub name: &'static str,
    pub protocol: Protocol,
    /// Smallest and largest operating range the firmware accepts, in degrees.
    pub range: (u16, u16),
    pub rpm_leds: u8,
}

/// Every wheel we know the command channel of.
pub const MODELS: &[WheelModel] = &[
    WheelModel { product_id: 0xc267, name: "G923 Racing Wheel", protocol: Protocol::ClassicReport30, range: (40, 900), rpm_leds: 5 },
    WheelModel { product_id: 0xc266, name: "G923 Racing Wheel", protocol: Protocol::Classic, range: (40, 900), rpm_leds: 5 },
    WheelModel { product_id: 0xc24f, name: "G29 Driving Force", protocol: Protocol::Classic, range: (40, 900), rpm_leds: 5 },
    WheelModel { product_id: 0xc29b, name: "G27 Racing Wheel", protocol: Protocol::Classic, range: (40, 900), rpm_leds: 5 },
    WheelModel { product_id: 0xc299, name: "G25 Racing Wheel", protocol: Protocol::Classic, range: (40, 900), rpm_leds: 0 },
    WheelModel { product_id: 0xc29a, name: "Driving Force GT", protocol: Protocol::Classic, range: (40, 900), rpm_leds: 0 },
    WheelModel { product_id: 0xc298, name: "Driving Force Pro", protocol: Protocol::Classic, range: (40, 900), rpm_leds: 0 },
    WheelModel { product_id: 0xc294, name: "Driving Force", protocol: Protocol::Classic, range: (40, 270), rpm_leds: 0 },
    WheelModel { product_id: 0xc295, name: "MOMO Force", protocol: Protocol::Classic, range: (40, 270), rpm_leds: 0 },
    WheelModel { product_id: 0xca03, name: "MOMO Racing", protocol: Protocol::Classic, range: (40, 270), rpm_leds: 0 },
    WheelModel { product_id: 0xc262, name: "G920 Driving Force", protocol: Protocol::Hidpp, range: (180, 900), rpm_leds: 0 },
    WheelModel { product_id: 0xc26e, name: "G923 Racing Wheel", protocol: Protocol::Hidpp, range: (180, 900), rpm_leds: 5 },
];

pub fn model_for(pid: u16) -> Option<&'static WheelModel> {
    MODELS.iter().find(|m| m.product_id == pid)
}

/// Wheels driven here rather than through HID++.
pub fn is_classic(pid: u16) -> bool {
    matches!(model_for(pid), Some(m) if m.protocol != Protocol::Hidpp)
}

// ---------------------------------------------------------------------------
// Live state
// ---------------------------------------------------------------------------

/// What the wheel reports, normalised: steering −1..1, pedals 0..1.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelState {
    /// Raw 16-bit steering position, 0x8000 = centre.
    pub steering_raw: u16,
    /// −1.0 (full left) … 1.0 (full right), after centre calibration.
    pub steering: f32,
    pub accelerator: f32,
    pub brake: f32,
    pub clutch: f32,
    /// Bitmask; bit n = button n+1.
    pub buttons: u32,
    /// 0-7 clockwise from up, 8 = released.
    pub hat: u8,
}

impl WheelState {
    /// Steering angle in degrees for the given operating range.
    pub fn angle(&self, range_deg: u16) -> f32 {
        self.steering * range_deg as f32 / 2.0
    }
}

/// Parses an input report from a wheel in G923 PS mode (report id 1, 63 bytes).
///
/// Layout, all captured on hardware (2026-09-18): steering u16 LE at 43,
/// pedals u16 LE at 45/47/49 with 0xFFFF released; hat in byte 5's low
/// nibble; buttons in DualShock order — byte 5 high nibble □ ✕ ○ △, byte 6
/// L1/R1 (the paddles), L2, R2, Share, Options, L3, R3, byte 7 bit 0 PS;
/// the wheel's own extras in byte 54 — bit 0 Enter (dial press), 1 dial
/// left, 2 dial right, 3 −, 4 +; the Driving Force Shifter in byte 51 —
/// bits 0-5 gears 1-6, bit 7 reverse.
///
/// The mask keeps that order: bits 0-12 DualShock, 13-17 the extras, 20-27
/// the shifter. `ghub_mask` re-numbers it the way G HUB does.
pub fn parse_ps_report(rep: &[u8]) -> Option<WheelState> {
    if rep.len() < 51 || rep[0] != 0x01 {
        return None;
    }
    let u16_at = |i: usize| u16::from_le_bytes([rep[i], rep[i + 1]]);
    let pedal = |i: usize| 1.0 - u16_at(i) as f32 / 65535.0;
    let raw = u16_at(43);
    let hat = rep[5] & 0x0f;
    let shifter = rep.get(51).copied().unwrap_or(0) as u32;
    let extras = (rep.get(54).copied().unwrap_or(0) & 0x1f) as u32;
    let buttons = ((rep[5] >> 4) as u32)
        | ((rep[6] as u32) << 4)
        | ((rep[7] & 0x01) as u32) << 12
        | (extras << EXTRAS_BIT)
        | (shifter << SHIFTER_BIT);
    Some(WheelState {
        steering_raw: raw,
        steering: (raw as f32 - 32768.0) / 32768.0,
        accelerator: pedal(45),
        brake: pedal(47),
        clutch: pedal(49),
        buttons,
        hat: if hat > 7 { 8 } else { hat },
    })
}

/// First button bit used by the H-pattern shifter: gears 1-6 follow, then an
/// unused bit, then reverse.
pub const SHIFTER_BIT: u32 = 20;
/// First bit of the wheel's extras: Enter, dial left, dial right, −, +.
pub const EXTRAS_BIT: u32 = 13;

/// How G HUB numbers the G923's controls (its `gN` slots, from the depot's
/// marker positions on the render): 1 ✕, 2 □, 3 ○, 4 △, 5 right paddle,
/// 6 left paddle, 7 R2, 8 L2, 9 Share, 10 Options, 11 R3, 12 L3, 13-18 gears
/// 1-6, 19 reverse, 20 +, 21 −, 22 dial right, 23 dial left, 24 Enter, 25 PS,
/// 26-33 D-pad up, up-right, right, down-right, down, down-left, left,
/// up-left. Assignments use these numbers (`button-N`).
pub const GHUB_BUTTON_COUNT: u8 = 33;

/// Raw mask bit for G HUB button `n` (1-based), `None` for the D-pad.
const GHUB_TO_RAW: [Option<u32>; 25] = [
    Some(1),  // 1 ✕
    Some(0),  // 2 □
    Some(2),  // 3 ○
    Some(3),  // 4 △
    Some(5),  // 5 right paddle (R1)
    Some(4),  // 6 left paddle (L1)
    Some(7),  // 7 R2
    Some(6),  // 8 L2
    Some(8),  // 9 Share
    Some(9),  // 10 Options
    Some(11), // 11 R3
    Some(10), // 12 L3
    Some(20), // 13 1st
    Some(21), // 14 2nd
    Some(22), // 15 3rd
    Some(23), // 16 4th
    Some(24), // 17 5th
    Some(25), // 18 6th
    Some(27), // 19 reverse
    Some(17), // 20 +
    Some(16), // 21 −
    Some(15), // 22 dial right
    Some(14), // 23 dial left
    Some(13), // 24 Enter
    Some(12), // 25 PS
];

/// The state's buttons re-numbered as G HUB does (bit n-1 = button n), with
/// the D-pad's eight directions as buttons 26-33 from the hat.
pub fn ghub_mask(s: &WheelState) -> u32 {
    let mut out = 0u32;
    for (i, raw) in GHUB_TO_RAW.iter().enumerate() {
        if let Some(bit) = raw {
            if s.buttons & (1 << bit) != 0 {
                out |= 1 << i;
            }
        }
    }
    if s.hat < 8 {
        out |= 1 << (25 + s.hat as u32);
    }
    out
}

/// The gear an H-pattern shifter is in: 1-6, `Some(0)` for reverse, `None`
/// in neutral.
pub fn gear(buttons: u32) -> Option<u8> {
    let s = (buttons >> SHIFTER_BIT) & 0xff;
    if s & 0x80 != 0 {
        return Some(0);
    }
    (0..6u8).find(|g| s & (1 << g) != 0).map(|g| g + 1)
}

/// Parses a native-mode report (G29 / G27 / G923 PC mode): hat in byte 0's
/// low nibble, buttons following, steering u16 LE at 4, 8-bit pedals at 6-8.
pub fn parse_native_report(rep: &[u8]) -> Option<WheelState> {
    if rep.len() < 9 {
        return None;
    }
    let raw = u16::from_le_bytes([rep[4], rep[5]]);
    let hat = rep[0] & 0x0f;
    let buttons = ((rep[0] >> 4) as u32) | ((rep[1] as u32) << 4) | ((rep[2] as u32) << 12) | ((rep[3] as u32) << 20);
    Some(WheelState {
        steering_raw: raw,
        steering: (raw as f32 - 32768.0) / 32768.0,
        accelerator: 1.0 - rep[6] as f32 / 255.0,
        brake: 1.0 - rep[7] as f32 / 255.0,
        clutch: 1.0 - rep[8] as f32 / 255.0,
        buttons,
        hat: if hat > 7 { 8 } else { hat },
    })
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// G HUB's Steering Wheel panel, persisted per profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelSettings {
    /// Operating range in degrees.
    #[serde(default = "default_range")]
    pub range_deg: u16,
    /// 0-100, 50 = linear. Applied to the virtual wheel's steering axis.
    #[serde(default = "default_fifty")]
    pub sensitivity: u8,
    /// 0-100 centering spring strength.
    #[serde(default = "default_spring")]
    pub center_spring: u8,
    /// Keep the spring on while a game plays its own force feedback.
    #[serde(default)]
    pub center_spring_in_ffb_games: bool,
    /// Global force-feedback gain, 0-100.
    #[serde(default = "default_hundred")]
    pub ffb_gain: u8,
    /// Software centre calibration: raw steering offset, ±.
    #[serde(default)]
    pub center_offset: i16,
    /// TrueForce values as G HUB stores them (torque / audio effects / apply
    /// from game). Kept for parity and sharing; the TrueForce stream itself
    /// needs Logitech's game SDK, which does not exist on Linux.
    #[serde(default = "default_hundred")]
    pub trueforce_torque: u8,
    #[serde(default = "default_hundred")]
    pub trueforce_audio: u8,
    #[serde(default = "default_true")]
    pub trueforce_game_control: bool,
    /// Per-pedal response, applied to the virtual axes (G HUB's Pedals panel).
    #[serde(default)]
    pub pedals: PedalSettings,
}

/// One pedal's response curve and dead zones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PedalCurve {
    /// 0-100, 50 = linear: below it the pedal responds more at the top of
    /// its travel, above it more at the start (G HUB's Low / Medium / High).
    #[serde(default = "default_fifty")]
    pub sensitivity: u8,
    /// Travel ignored at the start, 0-40 %.
    #[serde(default)]
    pub dead_zone_low: u8,
    /// Travel treated as fully pressed at the end, 0-40 %.
    #[serde(default)]
    pub dead_zone_high: u8,
    /// Reverse the axis (some sims expect it).
    #[serde(default)]
    pub inverted: bool,
}

impl Default for PedalCurve {
    fn default() -> Self {
        PedalCurve { sensitivity: 50, dead_zone_low: 0, dead_zone_high: 0, inverted: false }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PedalSettings {
    #[serde(default)]
    pub accelerator: PedalCurve,
    #[serde(default)]
    pub brake: PedalCurve,
    #[serde(default)]
    pub clutch: PedalCurve,
    /// Report accelerator and brake as one axis (G HUB's "combined pedals"):
    /// brake pulls it negative, accelerator pushes it positive.
    #[serde(default)]
    pub combined: bool,
}

impl PedalCurve {
    /// Maps raw 0..1 travel through the dead zones and the sensitivity curve.
    pub fn apply(&self, raw: f32) -> f32 {
        let lo = self.dead_zone_low.min(40) as f32 / 100.0;
        let hi = 1.0 - self.dead_zone_high.min(40) as f32 / 100.0;
        let span = (hi - lo).max(0.05);
        let t = ((raw - lo) / span).clamp(0.0, 1.0);
        let shaped = sensitivity_curve(t, self.sensitivity);
        if self.inverted { 1.0 - shaped } else { shaped }
    }
}

fn default_range() -> u16 {
    900
}
fn default_fifty() -> u8 {
    50
}
fn default_spring() -> u8 {
    20
}
fn default_hundred() -> u8 {
    100
}
fn default_true() -> bool {
    true
}

impl Default for WheelSettings {
    fn default() -> Self {
        WheelSettings {
            range_deg: 900,
            sensitivity: 50,
            center_spring: 20,
            center_spring_in_ffb_games: false,
            ffb_gain: 100,
            center_offset: 0,
            trueforce_torque: 100,
            trueforce_audio: 100,
            trueforce_game_control: true,
            pedals: PedalSettings::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Handle
// ---------------------------------------------------------------------------

/// An opened wheel: the joystick interface, which carries both the input
/// reports and the command channel.
pub struct WheelHandle {
    pub model: &'static WheelModel,
    pub path: String,
    pub serial: Option<String>,
    device: Arc<Mutex<HidDevice>>,
    /// The last state seen, shared with the reader / force-feedback thread.
    pub state: Arc<Mutex<WheelState>>,
    pub settings: Arc<Mutex<WheelSettings>>,
    /// Running force-feedback bridge, if enabled.
    bridge: Option<ffb::Bridge>,
}

/// A wheel visible on the bus, before opening.
#[derive(Debug, Clone)]
pub struct WheelEndpoint {
    pub model: &'static WheelModel,
    pub path: String,
    pub serial: Option<String>,
    pub hid_product: Option<String>,
}

/// Finds the joystick interface of every classic wheel on the bus.
pub fn enumerate(api: &HidApi) -> Vec<WheelEndpoint> {
    let mut out = Vec::new();
    for info in api.device_list() {
        if info.vendor_id() != LOGITECH_VID {
            continue;
        }
        let Some(model) = model_for(info.product_id()) else {
            continue;
        };
        if model.protocol == Protocol::Hidpp {
            continue;
        }
        // Generic desktop page, joystick (4) or gamepad (5) — the interface
        // that carries the 7-byte command report.
        let usage_ok = info.usage_page() == 0x01 && matches!(info.usage(), 0x04 | 0x05);
        if !usage_ok && info.interface_number() != 0 {
            continue;
        }
        let Some(path) = info.path().to_str().ok().map(str::to_owned) else {
            continue;
        };
        if out.iter().any(|e: &WheelEndpoint| e.path == path) {
            continue;
        }
        out.push(WheelEndpoint {
            model,
            path,
            serial: info.serial_number().map(str::to_owned),
            hid_product: info.product_string().map(str::to_owned),
        });
    }
    out
}

impl WheelHandle {
    pub fn open(api: &HidApi, endpoint: &WheelEndpoint) -> Result<Self> {
        let cpath = std::ffi::CString::new(endpoint.path.as_str())
            .map_err(|_| Error::other("hid path contained a NUL byte"))?;
        let device = api.open_path(&cpath)?;
        device.set_blocking_mode(false)?;
        Ok(WheelHandle {
            model: endpoint.model,
            path: endpoint.path.clone(),
            serial: endpoint.serial.clone(),
            device: Arc::new(Mutex::new(device)),
            state: Arc::new(Mutex::new(WheelState::default())),
            settings: Arc::new(Mutex::new(WheelSettings::default())),
            bridge: None,
        })
    }

    /// Stable id for the frontend, shaped like the HID++ ones.
    pub fn id(&self) -> String {
        format!("046d:{:04x}:wheel", self.model.product_id)
    }

    /// Sends one classic command, wrapping it in report 0x30 when the wheel
    /// wants that.
    pub fn command(&self, cmd: [u8; 7]) -> Result<()> {
        send_command(&self.device.lock(), self.model.protocol, cmd)
    }

    /// Operating range in degrees, clamped to what the firmware accepts.
    pub fn set_range(&self, degrees: u16) -> Result<u16> {
        let (lo, hi) = self.model.range;
        let d = degrees.clamp(lo, hi);
        self.command([0xf8, 0x81, (d & 0xff) as u8, (d >> 8) as u8, 0, 0, 0])?;
        Ok(d)
    }

    /// RPM LEDs, bit 0 = leftmost.
    pub fn set_leds(&self, mask: u8) -> Result<()> {
        if self.model.rpm_leds == 0 {
            return Err(Error::other("this wheel has no RPM LEDs"));
        }
        let mask = mask & ((1u8 << self.model.rpm_leds) - 1);
        self.command([0xf8, 0x12, mask, 0, 0, 0, 0])
    }

    /// Centering spring, 0 (off) … 100. Same maths as new-lg4ff's autocenter,
    /// on a 16-bit magnitude, for non-MOMO wheels.
    pub fn set_center_spring(&self, percent: u8) -> Result<()> {
        if percent == 0 {
            return self.command([0xf5, 0, 0, 0, 0, 0, 0]);
        }
        let magnitude = (percent.min(100) as u32 * 0xffff) / 100;
        let (a, b) = if magnitude <= 0xaaaa {
            (0x0c * magnitude, 0x80 * magnitude)
        } else {
            (0x0c * 0xaaaa + 0x06 * (magnitude - 0xaaaa), 0x80 * 0xaaaa + 0xff * (magnitude - 0xaaaa))
        };
        let a = a >> 1;
        self.command([0xfe, 0x0d, (a / 0xaaaa) as u8, (a / 0xaaaa) as u8, (b / 0xaaaa) as u8, 0, 0])?;
        self.command([0x14, 0, 0, 0, 0, 0, 0])
    }

    /// PS mode → native mode. The wheel drops off the bus and comes back with
    /// a different product id; not needed for anything G HUB does, but it is
    /// how a game expecting the G29-style report would want it.
    pub fn switch_to_native(&self) -> Result<()> {
        self.command([0xf8, 0x09, 0x07, 0x01, 0x01, 0x00, 0x00])
    }

    /// Reads whatever input reports are pending and updates the shared state.
    /// Returns the newest state, or `None` when nothing arrived.
    pub fn poll(&self) -> Option<WheelState> {
        if self.bridge.is_some() {
            return Some(*self.state.lock());
        }
        let dev = self.device.lock();
        let mut buf = [0u8; 64];
        let mut latest = None;
        loop {
            let n = dev.read_timeout(&mut buf, 0).unwrap_or(0);
            if n == 0 {
                break;
            }
            let parsed = match self.model.protocol {
                Protocol::ClassicReport30 => parse_ps_report(&buf[..n]),
                _ => parse_native_report(&buf[..n]),
            };
            if let Some(s) = parsed {
                latest = Some(s);
            }
        }
        if let Some(s) = latest {
            let calibrated = apply_settings(s, &self.settings.lock());
            *self.state.lock() = calibrated;
            return Some(calibrated);
        }
        None
    }

    /// Applies a full settings block to the hardware and remembers it.
    pub fn apply(&self, settings: &WheelSettings) -> Result<()> {
        let range = self.set_range(settings.range_deg)?;
        // While the force-feedback bridge runs, the spring is its business
        // (it is dropped when a game plays effects unless the user wants both).
        if self.bridge.is_none() {
            self.set_center_spring(settings.center_spring)?;
        }
        let mut current = self.settings.lock();
        *current = settings.clone();
        current.range_deg = range;
        drop(current);
        if let Some(b) = &self.bridge {
            b.update_settings(settings);
        }
        Ok(())
    }

    pub fn bridge_running(&self) -> bool {
        self.bridge.is_some()
    }

    /// Starts the userspace force-feedback driver for this wheel.
    pub fn start_bridge(&mut self, evdev_grab_paths: Vec<String>) -> Result<()> {
        if self.bridge.is_some() {
            return Ok(());
        }
        let bridge = ffb::Bridge::start(
            self.model,
            Arc::clone(&self.device),
            Arc::clone(&self.state),
            Arc::clone(&self.settings),
            evdev_grab_paths,
        )?;
        self.bridge = Some(bridge);
        Ok(())
    }

    pub fn stop_bridge(&mut self) {
        if let Some(b) = self.bridge.take() {
            b.stop();
            // Hand the spring back to the plain settings.
            let spring = self.settings.lock().center_spring;
            let _ = self.set_center_spring(spring);
        }
    }
}

/// Writes one classic command through the device's report channel.
pub fn send_command(dev: &HidDevice, protocol: Protocol, cmd: [u8; 7]) -> Result<()> {
    let bytes: Vec<u8> = match protocol {
        Protocol::ClassicReport30 => {
            let mut v = vec![0x30];
            v.extend_from_slice(&cmd);
            v
        }
        Protocol::Classic => {
            // hidraw needs a leading report id byte; 0 = no id.
            let mut v = vec![0x00];
            v.extend_from_slice(&cmd);
            v
        }
        Protocol::Hidpp => return Err(Error::other("HID++ wheels are not driven through this channel")),
    };
    dev.write(&bytes).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("No such file") || msg.contains("ENOENT") {
            Error::other(
                "the wheel's USB output endpoint is not usable on this controller (try another USB port, or the usbhid quirk from the README)",
            )
        } else {
            Error::Hid(e)
        }
    })?;
    Ok(())
}

/// Centre calibration and the sensitivity curve, applied to a raw state.
pub fn apply_settings(mut s: WheelState, settings: &WheelSettings) -> WheelState {
    let centred = (s.steering_raw as i32 - 32768 - settings.center_offset as i32).clamp(-32768, 32767);
    let linear = centred as f32 / 32768.0;
    s.steering = sensitivity_curve(linear, settings.sensitivity);
    s.accelerator = settings.pedals.accelerator.apply(s.accelerator);
    s.brake = settings.pedals.brake.apply(s.brake);
    s.clutch = settings.pedals.clutch.apply(s.clutch);
    s
}

/// G HUB's sensitivity slider: 50 is linear; below it the centre gets softer
/// (more turn for the same output), above it sharper. Implemented as a power
/// curve on the magnitude so the sign and the end stops are untouched.
pub fn sensitivity_curve(x: f32, sensitivity: u8) -> f32 {
    let s = sensitivity.min(100) as f32;
    // 0 → exponent 2 (soft), 50 → 1 (linear), 100 → 0.5 (sharp).
    let exponent = if s <= 50.0 { 2.0 - s / 50.0 } else { 1.0 - (s - 50.0) / 100.0 };
    x.signum() * x.abs().powf(exponent.max(0.5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_g923_ps_report() {
        let mut rep = vec![0u8; 63];
        rep[0] = 0x01;
        rep[5] = 0x28; // hat 8 (released) + square
        rep[43] = 0x00;
        rep[44] = 0x80; // centre
        for i in 45..51 {
            rep[i] = 0xff; // pedals released
        }
        rep[47] = 0x00;
        rep[48] = 0x00; // brake fully pressed
        let s = parse_ps_report(&rep).unwrap();
        assert_eq!(s.steering_raw, 0x8000);
        assert!(s.steering.abs() < 1e-6);
        assert!((s.brake - 1.0).abs() < 1e-6);
        assert!(s.accelerator.abs() < 1e-6);
        assert_eq!(s.hat, 8);
        assert_eq!(s.buttons & 0xf, 0x2);
        assert_eq!(gear(s.buttons), None);

        // Shifter byte: 3rd gear, then reverse.
        rep[51] = 0x04;
        let s = parse_ps_report(&rep).unwrap();
        assert_eq!(gear(s.buttons), Some(3));
        assert_eq!(s.buttons >> SHIFTER_BIT, 0x04);
        rep[51] = 0x80;
        assert_eq!(gear(parse_ps_report(&rep).unwrap().buttons), Some(0));
    }

    #[test]
    fn ghub_numbering_matches_the_capture() {
        let mut rep = vec![0u8; 63];
        rep[0] = 0x01;
        rep[44] = 0x80;
        for i in 45..51 {
            rep[i] = 0xff;
        }
        // Square + Enter + 3rd gear, hat up.
        rep[5] = 0x10;
        rep[54] = 0x01;
        rep[51] = 0x04;
        let s = parse_ps_report(&rep).unwrap();
        let m = ghub_mask(&s);
        let has = |n: u32| m & (1 << (n - 1)) != 0;
        assert!(has(2) && has(24) && has(15) && has(26), "{m:#x}");
        assert_eq!(m.count_ones(), 4);
        // Dial right and + live in byte 54.
        rep[5] = 0x08;
        rep[51] = 0;
        rep[54] = 0x14;
        let m = ghub_mask(&parse_ps_report(&rep).unwrap());
        assert_eq!(m, (1 << 21) | (1 << 19));
    }

    #[test]
    fn sensitivity_is_linear_at_fifty() {
        assert!((sensitivity_curve(0.5, 50) - 0.5).abs() < 1e-6);
        assert!(sensitivity_curve(0.5, 0) < 0.5);
        assert!(sensitivity_curve(0.5, 100) > 0.5);
        assert!((sensitivity_curve(-1.0, 100) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn pedal_curve_dead_zones_and_shape() {
        let c = PedalCurve { sensitivity: 50, dead_zone_low: 10, dead_zone_high: 10, inverted: false };
        assert!(c.apply(0.05).abs() < 1e-6, "inside the low dead zone");
        assert!((c.apply(0.95) - 1.0).abs() < 1e-6, "inside the high dead zone");
        assert!((c.apply(0.5) - 0.5).abs() < 1e-6, "linear in the middle");
        let soft = PedalCurve { sensitivity: 20, ..Default::default() };
        assert!(soft.apply(0.5) < 0.5);
        let inv = PedalCurve { inverted: true, ..Default::default() };
        assert!((inv.apply(0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn centre_offset_shifts_the_axis() {
        let s = WheelState { steering_raw: 0x8000 + 200, ..Default::default() };
        let settings = WheelSettings { center_offset: 200, ..Default::default() };
        assert!(apply_settings(s, &settings).steering.abs() < 1e-6);
    }

    #[test]
    fn model_table_covers_the_test_wheel() {
        let m = model_for(0xc267).unwrap();
        assert_eq!(m.protocol, Protocol::ClassicReport30);
        assert!(is_classic(0xc267));
        assert!(!is_classic(0xc262));
    }
}
