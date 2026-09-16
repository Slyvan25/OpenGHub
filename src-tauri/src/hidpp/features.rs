//! Typed wrappers over the individual HID++ 2.0 features.
//!
//! Each submodule holds the feature id, its function ids and a small set of
//! helpers that turn raw [`Packet`] parameters into ordinary Rust values.

use serde::{Deserialize, Serialize};

use super::{Error, Handle, Packet, ReportKind, Result};

// ---------------------------------------------------------------------------
// 0x0000 Root — always at index 0, resolves every other feature.
// ---------------------------------------------------------------------------
pub mod root {
    pub const ID: u16 = 0x0000;
    /// The Root feature is the one feature whose index is fixed by the spec.
    pub const INDEX: u8 = 0x00;
    pub const FN_GET_FEATURE: u8 = 0x00;
    pub const FN_GET_PROTOCOL_VERSION: u8 = 0x01;
}

// ---------------------------------------------------------------------------
// 0x0001 FeatureSet — enumerate everything the device supports.
// ---------------------------------------------------------------------------
pub mod feature_set {
    pub const ID: u16 = 0x0001;
    pub const FN_GET_COUNT: u8 = 0x00;
    pub const FN_GET_FEATURE_ID: u8 = 0x01;
}

// ---------------------------------------------------------------------------
// 0x0003 DeviceInformation / 0x0005 DeviceNameAndType
// ---------------------------------------------------------------------------
pub mod device_name {
    pub const ID: u16 = 0x0005;
    pub const FN_GET_COUNT: u8 = 0x00;
    pub const FN_GET_NAME: u8 = 0x01;
    pub const FN_GET_TYPE: u8 = 0x02;
}

pub mod device_info {
    pub const ID: u16 = 0x0003;
    pub const FN_GET_DEVICE_INFO: u8 = 0x00;
    pub const FN_GET_FW_INFO: u8 = 0x01;
}

/// One firmware entity from `0x0003` `getFwInfo`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareInfo {
    /// `main`, `bootloader`, `hardware`, `optical sensor`, … as G HUB labels them.
    pub kind: String,
    /// e.g. `MPM17.00_B0008` — prefix, BCD number.revision, build.
    pub version: String,
    pub active: bool,
}

/// Every firmware entity the device lists. Verified on a G502 LIGHTSPEED:
/// `MPM17.00_B0008` (main, active) and `BOT92.00_B0008` (bootloader).
pub fn read_firmware(h: &mut Handle) -> Result<Vec<FirmwareInfo>> {
    let idx = h.feature_index(device_info::ID)?;
    let count = h.call(idx, device_info::FN_GET_DEVICE_INFO, &[], ReportKind::Long)?.param(0);
    let mut out = Vec::new();
    for e in 0..count.min(8) {
        let p = h.call(idx, device_info::FN_GET_FW_INFO, &[e], ReportKind::Long)?;
        let kind = match p.param(0) {
            0 => "main",
            1 => "bootloader",
            2 => "hardware",
            3 => "touchpad",
            4 => "optical sensor",
            5 => "softdevice",
            6 => "RF companion",
            7 => "factory app",
            8 => "RGB custom effect",
            9 => "motor drive",
            _ => "other",
        };
        let prefix: String = (1..4).map(|i| p.param(i)).filter(|c| c.is_ascii_graphic()).map(|c| c as char).collect();
        let version = format!("{prefix}{:02x}.{:02x}_B{:04x}", p.param(4), p.param(5), p.param_u16(6));
        out.push(FirmwareInfo { kind: kind.into(), version, active: p.param(8) & 1 == 1 });
    }
    Ok(out)
}

/// The device's own product ids, from `0x0003` `getDeviceInfo`.
///
/// Reached through a receiver, the USB product id belongs to the *dongle*, not
/// the device — a G502 on a LIGHTSPEED receiver looks like `0xc539`. `modelId`
/// carries the device's real ids instead: up to three big-endian u16s, one per
/// supported transport, e.g. `407f c08d 0000` for the eQuad and USB ids of the
/// same mouse.
pub fn read_model_ids(h: &mut Handle) -> Result<Vec<u16>> {
    let idx = h.feature_index(device_info::ID)?;
    let reply = h.call(idx, device_info::FN_GET_DEVICE_INFO, &[], ReportKind::Long)?;
    // entityCnt(1) + unitId(4) + transport(2), then modelId(6).
    Ok((0..3).map(|i| reply.param_u16(7 + i * 2)).filter(|id| *id != 0).collect())
}

// ---------------------------------------------------------------------------
// Battery: 0x1000 (level/status), 0x1001 (voltage), 0x1004 (unified)
// ---------------------------------------------------------------------------
pub mod battery {
    pub const ID_LEVEL_STATUS: u16 = 0x1000;
    pub const ID_VOLTAGE: u16 = 0x1001;
    pub const ID_UNIFIED: u16 = 0x1004;

    pub const FN_GET_LEVEL_STATUS: u8 = 0x00;
    pub const FN_GET_CAPABILITY: u8 = 0x01;

    pub const FN_VOLTAGE_GET: u8 = 0x00;

    pub const FN_UNIFIED_GET_CAPABILITIES: u8 = 0x00;
    pub const FN_UNIFIED_GET_STATUS: u8 = 0x01;
}

// ---------------------------------------------------------------------------
// 0x2201 AdjustableDPI
// ---------------------------------------------------------------------------
pub mod dpi {
    pub const ID: u16 = 0x2201;
    pub const FN_GET_SENSOR_COUNT: u8 = 0x00;
    pub const FN_GET_SENSOR_DPI_LIST: u8 = 0x01;
    pub const FN_GET_SENSOR_DPI: u8 = 0x02;
    pub const FN_SET_SENSOR_DPI: u8 = 0x03;
}

// ---------------------------------------------------------------------------
// 0x8060 AdjustableReportRate / 0x8061 ExtendedAdjustableReportRate
// ---------------------------------------------------------------------------
pub mod report_rate {
    pub const ID: u16 = 0x8060;
    pub const FN_GET_LIST: u8 = 0x00;
    pub const FN_GET: u8 = 0x01;
    pub const FN_SET: u8 = 0x02;

    pub const ID_EXTENDED: u16 = 0x8061;
    pub const FN_EXT_GET_CAPABILITIES: u8 = 0x00;
    pub const FN_EXT_GET: u8 = 0x01;
    pub const FN_EXT_SET: u8 = 0x02;

    /// 0x8061 encodes rates as an index into this table (divisors of 8 ms).
    pub const EXTENDED_RATES_HZ: [u32; 7] = [125, 250, 500, 1000, 2000, 4000, 8000];
}

// ---------------------------------------------------------------------------
// 0x8100 OnboardProfiles
// ---------------------------------------------------------------------------
pub mod onboard {
    pub const ID: u16 = 0x8100;
    pub const FN_SET_MODE: u8 = 0x01;
    pub const FN_GET_MODE: u8 = 0x02;

    /// The device runs its own stored profile and owns the LEDs.
    pub const MODE_ONBOARD: u8 = 0x01;
    /// Software drives the device; required before `0x8070` writes are visible.
    pub const MODE_HOST: u8 = 0x02;
}

// ---------------------------------------------------------------------------
// 0x8110 Mouse Button Spy — G HUB's software-mode assignments
// ---------------------------------------------------------------------------
pub mod button_spy {
    pub const ID: u16 = 0x8110;
    pub const FN_GET_NB_OF_BUTTONS: u8 = 0x00;
    pub const FN_START_SPY: u8 = 0x01;
    pub const FN_STOP_SPY: u8 = 0x02;
    pub const FN_GET_REMAPPING: u8 = 0x03;
    pub const FN_SET_REMAPPING: u8 = 0x04;
    /// Event 0: a 16-bit big-endian mask of pressed buttons (bit 0 = button 1).
    pub const EVENT_BUTTON_REPORT: u8 = 0x00;
    /// Remapping a button to this makes the device send no HID action for it,
    /// leaving the spy report as the only trace — which is what software
    /// assignments want.
    pub const NO_HID_ACTION: u8 = 0x00;
}

// ---------------------------------------------------------------------------
// 0x8070 ColorLedEffects / 0x8071 RGBEffects
// ---------------------------------------------------------------------------
pub mod lighting {
    pub const ID_COLOR_LED_EFFECTS: u16 = 0x8070;
    pub const FN_GET_INFO: u8 = 0x00;
    pub const FN_GET_ZONE_INFO: u8 = 0x01;
    pub const FN_GET_ZONE_EFFECT_INFO: u8 = 0x02;
    pub const FN_SET_ZONE_EFFECT: u8 = 0x03;
    pub const FN_GET_ZONE_EFFECT: u8 = 0x0e;

    pub const ID_RGB_EFFECTS: u16 = 0x8071;
    pub const FN_RGB_GET_INFO: u8 = 0x00;
    pub const FN_RGB_SET_EFFECT: u8 = 0x03;
    pub const FN_RGB_SET_CONTROL_MODE: u8 = 0x05;

    /// Effect ids, as reported by `getZoneEffectInfo`. Note breathing is `0x0a`,
    /// not `0x02` — the ids are not contiguous.
    pub const EFFECT_OFF: u8 = 0x00;
    pub const EFFECT_FIXED: u8 = 0x01;
    pub const EFFECT_CYCLE: u8 = 0x03;
    pub const EFFECT_BREATHING: u8 = 0x0a;

    /// Passed as the zone index to mean "every zone on the device".
    pub const ALL_ZONES: u8 = 0xff;

    /// Trailing byte of `setZoneEffect`: 0 applies to RAM only (lost on replug),
    /// 1 also writes the device's flash so it survives a power cycle.
    pub const PERSIST_RAM: u8 = 0x00;
    pub const PERSIST_RAM_AND_FLASH: u8 = 0x01;
}

// ---------------------------------------------------------------------------
// Returned values
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChargeStatus {
    Discharging,
    Charging,
    ChargingFull,
    Full,
    SlowCharging,
    Error,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatteryState {
    /// 0-100. Devices that only report coarse levels are mapped onto this scale.
    pub percentage: u8,
    /// True when the device only reports buckets (critical/low/good/full) and the
    /// percentage above is an approximation.
    pub approximate: bool,
    pub status: ChargeStatus,
    pub voltage_mv: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DpiState {
    pub sensor: u8,
    pub current: u16,
    pub default: u16,
    /// The discrete values a list-type sensor offers. Empty for range sensors,
    /// which are fully described by `min`/`max`/`step` — expanding those into a
    /// list would be thousands of numbers over IPC that nothing reads.
    pub steps: Vec<u16>,
    pub min: u16,
    pub max: u16,
    pub step: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportRateState {
    pub current_hz: u32,
    pub available_hz: Vec<u32>,
    /// True when driven through 0x8061 rather than 0x8060.
    pub extended: bool,
}

// ---------------------------------------------------------------------------
// Implementations
// ---------------------------------------------------------------------------

/// Reads the device's own name via `0x0005`, in 16 byte chunks.
pub fn read_device_name(h: &mut Handle) -> Result<String> {
    let idx = h.feature_index(device_name::ID)?;
    let len = h.call(idx, device_name::FN_GET_COUNT, &[], ReportKind::Short)?.param(0) as usize;
    let mut name = String::with_capacity(len);
    let mut offset = 0usize;
    while offset < len && offset < 64 {
        let chunk = h.call(idx, device_name::FN_GET_NAME, &[offset as u8], ReportKind::Long)?;
        let mut got = 0;
        for b in &chunk.params {
            if *b == 0 || name.len() >= len {
                break;
            }
            name.push(*b as char);
            got += 1;
        }
        if got == 0 {
            break;
        }
        offset += got;
    }
    let name = name.trim().to_string();
    if name.is_empty() {
        Err(Error::Malformed)
    } else {
        Ok(name)
    }
}

/// Device type byte from `0x0005` function 2.
pub fn read_device_type(h: &mut Handle) -> Result<u8> {
    let idx = h.feature_index(device_name::ID)?;
    Ok(h.call(idx, device_name::FN_GET_TYPE, &[], ReportKind::Short)?.param(0))
}

/// Reads battery state, preferring the modern unified feature and degrading to
/// the older level/status and raw voltage features.
pub fn read_battery(h: &mut Handle) -> Result<BatteryState> {
    if let Ok(state) = read_unified_battery(h) {
        return Ok(state);
    }
    if let Ok(state) = read_battery_level_status(h) {
        return Ok(state);
    }
    read_battery_voltage(h)
}

fn read_unified_battery(h: &mut Handle) -> Result<BatteryState> {
    let idx = h.feature_index(battery::ID_UNIFIED)?;
    let caps = h.call(idx, battery::FN_UNIFIED_GET_CAPABILITIES, &[], ReportKind::Short)?;
    // capability flags bit 1 = state-of-charge supported, bit 0 = coarse levels.
    let has_soc = caps.param(1) & 0x02 != 0;
    let status = h.call(idx, battery::FN_UNIFIED_GET_STATUS, &[], ReportKind::Short)?;

    let soc = status.param(0);
    let level_mask = status.param(1);
    let charging = status.param(2);

    let (percentage, approximate) = if has_soc && soc > 0 {
        (soc.min(100), false)
    } else {
        (level_mask_to_percent(level_mask), true)
    };

    Ok(BatteryState {
        percentage,
        approximate,
        status: match charging {
            0 => ChargeStatus::Discharging,
            1 => ChargeStatus::Charging,
            2 => ChargeStatus::ChargingFull,
            3 => ChargeStatus::Full,
            4 => ChargeStatus::SlowCharging,
            5 | 6 => ChargeStatus::Error,
            _ => ChargeStatus::Unknown,
        },
        voltage_mv: None,
    })
}

/// `0x1004` reports coarse levels as a bitmask. Bit 0 is the *lowest* bucket:
/// `1` critical, `2` low, `4` good, `8` full. The percentages are the midpoints
/// Solaar uses, since the device gives us nothing finer.
fn level_mask_to_percent(mask: u8) -> u8 {
    match mask {
        m if m & 0x08 != 0 => 90, // full
        m if m & 0x04 != 0 => 50, // good
        m if m & 0x02 != 0 => 20, // low
        m if m & 0x01 != 0 => 5,  // critical
        _ => 0,
    }
}

fn read_battery_level_status(h: &mut Handle) -> Result<BatteryState> {
    let idx = h.feature_index(battery::ID_LEVEL_STATUS)?;
    let reply = h.call(idx, battery::FN_GET_LEVEL_STATUS, &[], ReportKind::Short)?;
    let level = reply.param(0);
    let status = reply.param(2);
    // Devices with a discrete gauge report the number of levels here; when the
    // level is already a percentage `next_level` is the next step down.
    let capability = h.call(idx, battery::FN_GET_CAPABILITY, &[], ReportKind::Short).ok();
    let levels = capability.as_ref().map(|c| c.param(0)).unwrap_or(0);
    let approximate = levels > 0 && levels <= 8;

    Ok(BatteryState {
        percentage: level.min(100),
        approximate,
        status: match status {
            0 => ChargeStatus::Discharging,
            1 => ChargeStatus::Charging,
            2 => ChargeStatus::ChargingFull,
            3 => ChargeStatus::Full,
            4 => ChargeStatus::SlowCharging,
            5..=7 => ChargeStatus::Error,
            _ => ChargeStatus::Unknown,
        },
        voltage_mv: None,
    })
}

fn read_battery_voltage(h: &mut Handle) -> Result<BatteryState> {
    let idx = h.feature_index(battery::ID_VOLTAGE)?;
    let reply = h.call(idx, battery::FN_VOLTAGE_GET, &[], ReportKind::Short)?;
    let mv = reply.param_u16(0);
    let flags = reply.param(2);
    Ok(BatteryState {
        percentage: voltage_to_percent(mv),
        approximate: true,
        status: if flags & 0x80 != 0 { ChargeStatus::Charging } else { ChargeStatus::Discharging },
        voltage_mv: Some(mv),
    })
}

/// Rough Li-ion discharge curve (3.5 V empty → 4.2 V full), used only when the
/// device exposes nothing better than raw voltage.
fn voltage_to_percent(mv: u16) -> u8 {
    const EMPTY: f32 = 3500.0;
    const FULL: f32 = 4186.0;
    let clamped = (mv as f32).clamp(EMPTY, FULL);
    (((clamped - EMPTY) / (FULL - EMPTY)) * 100.0).round() as u8
}

/// Reads DPI for `sensor`, including the selectable list.
pub fn read_dpi(h: &mut Handle, sensor: u8) -> Result<DpiState> {
    let idx = h.feature_index(dpi::ID)?;
    let current = h.call(idx, dpi::FN_GET_SENSOR_DPI, &[sensor], ReportKind::Short)?;
    let list = h.call(idx, dpi::FN_GET_SENSOR_DPI_LIST, &[sensor], ReportKind::Long)?;
    let (steps, min, max, step) = parse_dpi_list(&list);

    Ok(DpiState {
        sensor,
        current: current.param_u16(1),
        default: current.param_u16(3),
        steps,
        min,
        max,
        step,
    })
}

pub fn sensor_count(h: &mut Handle) -> Result<u8> {
    let idx = h.feature_index(dpi::ID)?;
    Ok(h.call(idx, dpi::FN_GET_SENSOR_COUNT, &[], ReportKind::Short)?.param(0).max(1))
}

/// The DPI list is a sequence of big-endian u16 starting at param 1. A value
/// with the top three bits set (`0xE000`) is not a DPI but the *step size* of a
/// range whose bounds are the preceding and following entries.
fn parse_dpi_list(reply: &Packet) -> (Vec<u16>, u16, u16, u16) {
    let mut values = Vec::new();
    let mut step_marker: Option<u16> = None;
    let mut i = 1;
    while i + 1 < reply.params.len() {
        let raw = reply.param_u16(i);
        i += 2;
        if raw == 0 {
            break;
        }
        if raw & 0xe000 == 0xe000 {
            step_marker = Some(raw & 0x1fff);
        } else {
            values.push(raw);
        }
    }

    if values.is_empty() {
        return (Vec::new(), 0, 0, 0);
    }

    match step_marker {
        // Range form: [min, step-marker, max]. Reported as bounds, not a list.
        Some(step) if step > 0 && values.len() >= 2 => {
            let min = values[0];
            let max = *values.last().unwrap();
            (Vec::new(), min, max, step)
        }
        // Discrete list form.
        _ => {
            let min = *values.iter().min().unwrap();
            let max = *values.iter().max().unwrap();
            values.sort_unstable();
            values.dedup();
            (values, min, max, 0)
        }
    }
}

/// Writes a new DPI, clamped and snapped to what the sensor actually accepts.
pub fn write_dpi(h: &mut Handle, sensor: u8, requested: u16) -> Result<u16> {
    let state = read_dpi(h, sensor)?;
    let target = snap_dpi(&state, requested);
    let [hi, lo] = target.to_be_bytes();
    let idx = h.feature_index(dpi::ID)?;
    with_host_mode(h, |h| {
        h.call(idx, dpi::FN_SET_SENSOR_DPI, &[sensor, hi, lo], ReportKind::Short)
    })?;
    Ok(target)
}

fn snap_dpi(state: &DpiState, requested: u16) -> u16 {
    if state.min == 0 && state.max == 0 {
        return requested;
    }
    let clamped = requested.clamp(state.min, state.max);
    if state.step > 0 {
        // Range sensor: round to the nearest multiple of `step` above `min`.
        let offset = clamped.saturating_sub(state.min);
        let snapped = state.min + ((offset + state.step / 2) / state.step) * state.step;
        return snapped.min(state.max);
    }
    // Discrete sensor: pick the closest offered value.
    state
        .steps
        .iter()
        .copied()
        .min_by_key(|v| v.abs_diff(clamped))
        .unwrap_or(clamped)
}

/// Reads the report rate, preferring `0x8061` (supports 2/4/8 kHz) over `0x8060`.
pub fn read_report_rate(h: &mut Handle) -> Result<ReportRateState> {
    if let Ok(idx) = h.feature_index(report_rate::ID_EXTENDED) {
        // connection type 0 = wired, 1 = wireless; ask about the active one.
        let caps = h.call(idx, report_rate::FN_EXT_GET_CAPABILITIES, &[0], ReportKind::Short)?;
        let mask = caps.param(0);
        let current_idx = h.call(idx, report_rate::FN_EXT_GET, &[], ReportKind::Short)?.param(0);
        let available: Vec<u32> = (0..report_rate::EXTENDED_RATES_HZ.len())
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| report_rate::EXTENDED_RATES_HZ[i])
            .collect();
        let current_hz = report_rate::EXTENDED_RATES_HZ
            .get(current_idx as usize)
            .copied()
            .unwrap_or(1000);
        if !available.is_empty() {
            return Ok(ReportRateState { current_hz, available_hz: available, extended: true });
        }
    }

    let idx = h.feature_index(report_rate::ID)?;
    let mask = h.call(idx, report_rate::FN_GET_LIST, &[], ReportKind::Short)?.param(0);
    let current_ms = h.call(idx, report_rate::FN_GET, &[], ReportKind::Short)?.param(0);
    // Bit N set => a period of (N + 1) ms is supported.
    let mut available: Vec<u32> = (0..8u8)
        .filter(|n| mask & (1 << n) != 0)
        .map(|n| 1000 / (n as u32 + 1))
        .collect();
    available.sort_unstable();
    Ok(ReportRateState {
        current_hz: if current_ms == 0 { 1000 } else { 1000 / current_ms as u32 },
        available_hz: available,
        extended: false,
    })
}

/// Sets the report rate in Hz. Picks the closest supported rate.
pub fn write_report_rate(h: &mut Handle, hz: u32) -> Result<u32> {
    let state = read_report_rate(h)?;
    let target = state
        .available_hz
        .iter()
        .copied()
        .min_by_key(|v| v.abs_diff(hz))
        .ok_or(Error::UnsupportedFeature(report_rate::ID))?;

    if state.extended {
        let index = report_rate::EXTENDED_RATES_HZ
            .iter()
            .position(|r| *r == target)
            .ok_or(Error::Malformed)? as u8;
        let idx = h.feature_index(report_rate::ID_EXTENDED)?;
        with_host_mode(h, |h| h.call(idx, report_rate::FN_EXT_SET, &[index], ReportKind::Short))?;
    } else {
        let period_ms = (1000 / target.max(1)).clamp(1, 8) as u8;
        let idx = h.feature_index(report_rate::ID)?;
        with_host_mode(h, |h| h.call(idx, report_rate::FN_SET, &[period_ms], ReportKind::Short))?;
    }
    Ok(target)
}

/// Number of physical buttons the spy can report.
pub fn spy_button_count(h: &mut Handle) -> Result<u8> {
    let idx = h.feature_index(button_spy::ID)?;
    Ok(h.call(idx, button_spy::FN_GET_NB_OF_BUTTONS, &[], ReportKind::Short)?.param(0))
}

/// Starts (or stops) raw button reporting. Verified on a G502 LIGHTSPEED:
/// reports arrive as long packets, function 0, params `[hi, lo]`.
pub fn set_spy(h: &mut Handle, on: bool) -> Result<()> {
    let idx = h.feature_index(button_spy::ID)?;
    let f = if on { button_spy::FN_START_SPY } else { button_spy::FN_STOP_SPY };
    h.call(idx, f, &[], ReportKind::Short).map(|_| ())
}

/// The device's button → HID action table: entry `i` is what physical button
/// `i + 1` does (its own number = default, 0 = nothing).
pub fn read_remapping(h: &mut Handle) -> Result<[u8; 16]> {
    let idx = h.feature_index(button_spy::ID)?;
    let p = h.call(idx, button_spy::FN_GET_REMAPPING, &[], ReportKind::Long)?;
    let mut out = [0u8; 16];
    for (i, b) in p.params.iter().take(16).enumerate() {
        out[i] = *b;
    }
    Ok(out)
}

pub fn write_remapping(h: &mut Handle, table: &[u8; 16]) -> Result<()> {
    let idx = h.feature_index(button_spy::ID)?;
    h.call(idx, button_spy::FN_SET_REMAPPING, table, ReportKind::Long).map(|_| ())
}

/// Decodes a spy notification into the pressed-button mask, if `packet` is one.
pub fn spy_event_mask(packet: &crate::hidpp::Packet, spy_index: u8) -> Option<u16> {
    if packet.feature_index != spy_index
        || packet.function_id() != button_spy::EVENT_BUTTON_REPORT
        || !packet.is_notification()
    {
        return None;
    }
    Some(u16::from_be_bytes([packet.param(0), packet.param(1)]))
}

/// Reads whether the device is running its onboard profile or is host-driven.
pub fn read_onboard_mode(h: &mut Handle) -> Result<u8> {
    let idx = h.feature_index(onboard::ID)?;
    Ok(h.call(idx, onboard::FN_GET_MODE, &[], ReportKind::Short)?.param(0))
}

/// Switches between onboard and host control.
pub fn write_onboard_mode(h: &mut Handle, mode: u8) -> Result<()> {
    let idx = h.feature_index(onboard::ID)?;
    h.call(idx, onboard::FN_SET_MODE, &[mode], ReportKind::Short)?;
    Ok(())
}

/// Runs a write and, if the device refuses because its onboard profile owns the
/// setting, takes host mode and tries once more.
///
/// A device running its onboard profile rejects `setReportRate` (and others)
/// with "invalid argument", which reads like a bad parameter but is really "not
/// yours to change". Host mode is **not persistent** — the device reverts on
/// reconnect — so this has to be handled per write rather than once at startup.
///
/// The mode is only changed when a write actually fails, so a device that is
/// happy in onboard mode is left alone.
fn with_host_mode<T>(
    h: &mut Handle,
    mut op: impl FnMut(&mut Handle) -> Result<T>,
) -> Result<T> {
    match op(h) {
        Err(Error::Protocol(e)) if e.is_invalid_argument() => {
            if !h.supports(onboard::ID) {
                return Err(Error::Protocol(e));
            }
            // Already host-driven? Then the argument really was invalid.
            if matches!(read_onboard_mode(h), Ok(onboard::MODE_HOST)) {
                return Err(Error::Protocol(e));
            }
            log::info!("device refused the write in onboard mode; taking host mode");
            write_onboard_mode(h, onboard::MODE_HOST)?;
            op(h)
        }
        other => other,
    }
}

/// Number of addressable lighting zones, via `0x8070` `getInfo`.
pub fn lighting_zone_count(h: &mut Handle) -> Result<u8> {
    let idx = h.feature_index(lighting::ID_COLOR_LED_EFFECTS)?;
    Ok(h.call(idx, lighting::FN_GET_INFO, &[0xff, 0x00, 0x00], ReportKind::Long)?.param(0))
}

/// What `getZoneInfo` reports about one lighting zone.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoneInfo {
    pub index: u8,
    /// Raw location code from the device.
    pub location: u16,
    /// Human name for that location — "Primary", "Logo", … — which is what
    /// G HUB shows as its per-zone tabs.
    pub location_name: String,
    /// Effect ids this zone accepts, in advertised order.
    pub effects: Vec<u8>,
}

/// Zone location codes, per the `0x8070` specification.
pub fn zone_location_name(location: u16) -> &'static str {
    match location {
        0x0001 => "Primary",
        0x0002 => "Logo",
        0x0003 => "Left",
        0x0004 => "Right",
        0x0005 => "Combined",
        0x0006 => "Primary 1",
        0x0007 => "Primary 2",
        0x0008 => "Primary 3",
        0x0009 => "Primary 4",
        0x000a => "Primary 5",
        0x000b => "Primary 6",
        _ => "Zone",
    }
}

/// Full description of every lighting zone on the device.
pub fn read_zones(h: &mut Handle) -> Result<Vec<ZoneInfo>> {
    let idx = h.feature_index(lighting::ID_COLOR_LED_EFFECTS)?;
    let count = lighting_zone_count(h)?;

    let mut zones = Vec::with_capacity(count as usize);
    for index in 0..count {
        let info = h.call(idx, lighting::FN_GET_ZONE_INFO, &[index], ReportKind::Long)?;
        let location = info.param_u16(1);
        let effect_count = info.param(3);

        let mut effects = Vec::with_capacity(effect_count as usize);
        for effect in 0..effect_count.min(16) {
            let reply =
                h.call(idx, lighting::FN_GET_ZONE_EFFECT_INFO, &[index, effect], ReportKind::Long)?;
            effects.push(reply.param(3));
        }

        zones.push(ZoneInfo {
            index,
            location,
            location_name: zone_location_name(location).to_string(),
            effects,
        });
    }
    Ok(zones)
}

/// The effect ids one zone actually accepts, from `getZoneEffectInfo`.
///
/// Worth querying rather than assuming: ids are not contiguous (a G502 reports
/// `0x00`, `0x01`, `0x03`, `0x0a`) and sending an unsupported one is rejected
/// with "invalid argument".
pub fn zone_effects(h: &mut Handle, zone: u8) -> Result<Vec<u8>> {
    let idx = h.feature_index(lighting::ID_COLOR_LED_EFFECTS)?;
    let info = h.call(idx, lighting::FN_GET_ZONE_INFO, &[zone], ReportKind::Long)?;
    let count = info.param(3);

    let mut effects = Vec::with_capacity(count as usize);
    for effect in 0..count.min(16) {
        let reply =
            h.call(idx, lighting::FN_GET_ZONE_EFFECT_INFO, &[zone, effect], ReportKind::Long)?;
        // The id is a big-endian u16, but every value in use fits in a byte.
        effects.push(reply.param(3));
    }
    Ok(effects)
}

/// Reads back the effect currently programmed into a zone, as `(mode, payload)`.
pub fn read_zone_effect(h: &mut Handle, zone: u8) -> Result<(u8, Vec<u8>)> {
    let idx = h.feature_index(lighting::ID_COLOR_LED_EFFECTS)?;
    let reply = h.call(idx, lighting::FN_GET_ZONE_EFFECT, &[zone], ReportKind::Long)?;
    Ok((reply.param(1), reply.params[2..].to_vec()))
}

/// Applies an effect to one zone, or to every zone when `zone` is
/// [`lighting::ALL_ZONES`].
///
/// `setZoneEffect` takes a fixed 13-byte payload:
///
/// ```text
/// [zone, effect_index, <10 bytes of effect-specific union>, persistence]
/// ```
///
/// Byte 1 is the **zone-local effect index**, not the global effect id. A G502
/// advertises ids `0x00, 0x01, 0x03, 0x0a` at indices `0..=3`, and accepts only
/// `0..=3` here — passing the id `0x0a` is rejected as "invalid argument", and
/// passing `0x03` silently selects index 3 (breathing) instead of cycle. So the
/// id is looked up in the zone's advertised list and translated to its index.
pub fn write_lighting(
    h: &mut Handle,
    zone: u8,
    rgb: [u8; 3],
    effect: LightEffect,
    persist: bool,
) -> Result<()> {
    let idx = h.feature_index(lighting::ID_COLOR_LED_EFFECTS)?;

    // A device running its onboard profile owns its own LEDs: 0x8070 writes are
    // accepted and then ignored. G HUB switches to host mode for the same
    // reason. Devices without 0x8100 simply skip this.
    if h.supports(onboard::ID) {
        match read_onboard_mode(h) {
            Ok(onboard::MODE_HOST) => {}
            Ok(_) => {
                if let Err(e) = write_onboard_mode(h, onboard::MODE_HOST) {
                    log::warn!("could not take host mode, lighting may not apply: {e}");
                }
            }
            Err(e) => log::debug!("could not read onboard mode: {e}"),
        }
    }

    let zones: Vec<u8> = if zone == lighting::ALL_ZONES {
        let count = lighting_zone_count(h)?;
        if count == 0 {
            return Err(Error::other("device reports no lighting zones"));
        }
        (0..count).collect()
    } else {
        vec![zone]
    };

    let (effect_id, union) = effect.encode(rgb);

    for z in zones {
        // The zone's effect list is ordered, so its position *is* the index the
        // device wants. This also rejects unsupported effects with a message
        // that says which ones exist, rather than a bare "invalid argument".
        let supported = zone_effects(h, z)?;
        let Some(effect_index) = supported.iter().position(|id| *id == effect_id) else {
            return Err(Error::other(format!(
                "zone {z} does not support effect {effect_id:#04x} (supports {})",
                supported.iter().map(|e| format!("{e:#04x}")).collect::<Vec<_>>().join(", ")
            )));
        };

        let mut params = [0u8; 13];
        params[0] = z;
        params[1] = effect_index as u8;
        params[2..12].copy_from_slice(&union);
        params[12] =
            if persist { lighting::PERSIST_RAM_AND_FLASH } else { lighting::PERSIST_RAM };

        h.call(idx, lighting::FN_SET_ZONE_EFFECT, &params, ReportKind::Long)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LightEffect {
    Off,
    Fixed,
    /// `rate_ms` is the full fade cycle; `brightness` 0-100.
    Breathing { rate_ms: u16, brightness: u8 },
    /// Full spectrum sweep; `rate_ms` is one revolution.
    Cycle { rate_ms: u16, brightness: u8 },
}

impl LightEffect {
    /// Returns `(effect_id, 10-byte union)`. The id is translated to the zone's
    /// local index by [`write_lighting`]; the union offsets follow the
    /// `hidpp20_internal_led` layout, starting right after the index byte.
    pub fn encode(self, rgb: [u8; 3]) -> (u8, [u8; 10]) {
        let mut p = [0u8; 10];
        match self {
            LightEffect::Off => (lighting::EFFECT_OFF, p),

            // fixed: { r, g, b, ramp, .. }
            LightEffect::Fixed => {
                p[0..3].copy_from_slice(&rgb);
                (lighting::EFFECT_FIXED, p)
            }

            // breathing: { r, g, b, period_be, waveform, intensity, .. }
            LightEffect::Breathing { rate_ms, brightness } => {
                p[0..3].copy_from_slice(&rgb);
                p[3..5].copy_from_slice(&rate_ms.to_be_bytes());
                p[6] = brightness;
                (lighting::EFFECT_BREATHING, p)
            }

            // cycle: { <5 bytes unused>, period_be, intensity, .. }
            LightEffect::Cycle { rate_ms, brightness } => {
                p[5..7].copy_from_slice(&rate_ms.to_be_bytes());
                p[7] = brightness;
                (lighting::EFFECT_CYCLE, p)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hidpp::{ReportKind, REPORT_ID_LONG};

    fn long_reply(params: &[u8]) -> Packet {
        let mut raw = vec![REPORT_ID_LONG, 0xff, 0x0a, 0x1a];
        raw.extend_from_slice(params);
        raw.resize(super::super::LONG_LEN, 0);
        Packet::parse(&raw).unwrap()
    }

    #[test]
    fn parses_discrete_dpi_list() {
        // sensor 0, then 400/800/1600/3200
        let reply = long_reply(&[0x00, 0x01, 0x90, 0x03, 0x20, 0x06, 0x40, 0x0c, 0x80]);
        let (steps, min, max, step) = parse_dpi_list(&reply);
        assert_eq!(steps, vec![400, 800, 1600, 3200]);
        assert_eq!((min, max, step), (400, 3200, 0));
    }

    #[test]
    fn parses_ranged_dpi_list() {
        // sensor 0, min 200, step marker 0xE032 (50), max 1000
        let reply = long_reply(&[0x00, 0x00, 0xc8, 0xe0, 0x32, 0x03, 0xe8]);
        let (steps, min, max, step) = parse_dpi_list(&reply);
        assert_eq!((min, max, step), (200, 1000, 50));
        // A range is described by its bounds, not enumerated.
        assert!(steps.is_empty());
    }

    #[test]
    fn snaps_to_nearest_offered_value() {
        let discrete = DpiState {
            sensor: 0,
            current: 800,
            default: 800,
            steps: vec![400, 800, 1600, 3200],
            min: 400,
            max: 3200,
            step: 0,
        };
        assert_eq!(snap_dpi(&discrete, 900), 800);
        assert_eq!(snap_dpi(&discrete, 1500), 1600);
        assert_eq!(snap_dpi(&discrete, 60000), 3200);

        let ranged = DpiState {
            sensor: 0,
            current: 1600,
            default: 1600,
            steps: vec![],
            min: 200,
            max: 25600,
            step: 50,
        };
        assert_eq!(snap_dpi(&ranged, 1620), 1600);
        assert_eq!(snap_dpi(&ranged, 1630), 1650);
        assert_eq!(snap_dpi(&ranged, 100), 200);
    }

    #[test]
    fn coarse_battery_levels_are_ordered_low_bit_first() {
        // Bit 0 is critical, bit 3 is full — getting this backwards would show a
        // nearly flat battery as full.
        assert_eq!(level_mask_to_percent(0x08), 90);
        assert_eq!(level_mask_to_percent(0x04), 50);
        assert_eq!(level_mask_to_percent(0x02), 20);
        assert_eq!(level_mask_to_percent(0x01), 5);
        assert_eq!(level_mask_to_percent(0x00), 0);
        // Several bits set: the highest wins.
        assert_eq!(level_mask_to_percent(0x0f), 90);
    }

    #[test]
    fn voltage_curve_endpoints() {
        assert_eq!(voltage_to_percent(3400), 0);
        assert_eq!(voltage_to_percent(4200), 100);
        assert!((45..=55).contains(&voltage_to_percent(3843)));
    }

    #[test]
    fn zone_locations_match_ghub_tab_names() {
        // A G502 reports 0x0001 and 0x0002 for its two zones.
        assert_eq!(zone_location_name(0x0001), "Primary");
        assert_eq!(zone_location_name(0x0002), "Logo");
        assert_eq!(zone_location_name(0x00ff), "Zone");
    }

    #[test]
    fn invalid_argument_is_recognised() {
        use crate::hidpp::ProtocolError;
        // 0x02 is what a device returns when its onboard profile owns a setting.
        assert!(ProtocolError(0x02).is_invalid_argument());
        assert!(!ProtocolError(0x09).is_invalid_argument());
        assert!(!ProtocolError(0x00).is_invalid_argument());
    }

    #[test]
    fn light_effect_encoding() {
        let (id, p) = LightEffect::Fixed.encode([0xff, 0x00, 0x88]);
        assert_eq!(id, lighting::EFFECT_FIXED);
        assert_eq!(p.len(), 10, "the union is 10 bytes after the mode byte");
        assert_eq!(&p[0..3], &[0xff, 0x00, 0x88]);

        // Breathing is 0x0a, not 0x02 — a G502 rejects 0x02 as invalid argument.
        let (id, p) = LightEffect::Breathing { rate_ms: 2000, brightness: 80 }.encode([1, 2, 3]);
        assert_eq!(id, 0x0a);
        assert_eq!(&p[0..3], &[1, 2, 3]);
        assert_eq!(&p[3..5], &2000u16.to_be_bytes());
        assert_eq!(p[6], 80);

        let (id, p) = LightEffect::Cycle { rate_ms: 5000, brightness: 60 }.encode([9, 9, 9]);
        assert_eq!(id, lighting::EFFECT_CYCLE);
        assert_eq!(&p[5..7], &5000u16.to_be_bytes());
        assert_eq!(p[7], 60);
        assert_eq!(&p[0..5], &[0; 5], "cycle ignores the colour");

        let (id, p) = LightEffect::Off.encode([1, 2, 3]);
        assert_eq!(id, lighting::EFFECT_OFF);
        assert_eq!(p, [0u8; 10]);
    }

    #[test]
    fn report_kind_param_lengths() {
        assert_eq!(ReportKind::Short.params_len(), 3);
        assert_eq!(ReportKind::Long.params_len(), 16);
    }
}
