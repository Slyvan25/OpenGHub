//! Tauri IPC surface.
//!
//! Every command is `Result<_, Error>`; `Error` serialises to a plain string, so
//! the Svelte side gets a rejected promise with a readable message instead of an
//! opaque failure. Commands that talk to hardware run on a blocking thread —
//! hidapi's read/write are synchronous and would otherwise stall the async runtime.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::hidpp::features::{BatteryState, DpiState, LightEffect, ReportRateState};
use crate::hidpp::{Error, Result};
use crate::profiles::{Config, DeviceProfile, Profile, Settings, Store};
use crate::state::{DeviceManager, DeviceSnapshot, EmptyReason};

/// Emitted on the `devices-changed` channel after any rescan.
pub const EVENT_DEVICES: &str = "devices-changed";
/// Emitted per device by the background poller.
pub const EVENT_BATTERY: &str = "battery-update";
/// Emitted when a device state changes as a result of a write.
pub const EVENT_DEVICE_UPDATED: &str = "device-updated";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatteryEvent {
    pub device_id: String,
    pub battery: BatteryState,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceListPayload {
    pub devices: Vec<DeviceSnapshot>,
    /// True when the list is synthetic (no reachable hardware).
    pub demo: bool,
    /// Populated when hidapi itself could not start, e.g. missing permissions.
    pub error: Option<String>,
    /// Why the list is empty, when it is. Drives the dashboard's guidance.
    pub empty_reason: Option<EmptyReason>,
}

impl DeviceListPayload {
    fn build(manager: &DeviceManager, devices: Vec<DeviceSnapshot>) -> Self {
        DeviceListPayload {
            devices,
            demo: manager.is_demo(),
            error: manager.init_error(),
            empty_reason: manager.empty_reason(),
        }
    }
}

// ---------------------------------------------------------------------------
// Devices
// ---------------------------------------------------------------------------

/// Rescans the bus and returns everything we can talk to.
#[tauri::command]
pub async fn get_connected_devices(
    app: AppHandle,
    manager: State<'_, DeviceManager>,
    refresh: Option<bool>,
) -> Result<DeviceListPayload> {
    let devices = if refresh.unwrap_or(true) {
        manager.refresh()
    } else {
        manager.snapshots()
    };
    let payload = DeviceListPayload::build(&manager, devices);
    let _ = app.emit(EVENT_DEVICES, &payload);
    Ok(payload)
}

/// Full current state of one device: DPI and its steps, polling rate, battery.
#[tauri::command]
pub async fn get_device_state(
    manager: State<'_, DeviceManager>,
    device_id: String,
) -> Result<DeviceSnapshot> {
    manager.refresh_device(&device_id).or_else(|e| {
        // A sleeping wireless device should still render its cached card.
        manager.snapshot(&device_id).ok_or(e)
    })
}

/// Lists the HID++ features a device exposes. Diagnostic aid for unknown hardware.
#[tauri::command]
pub async fn get_device_features(
    manager: State<'_, DeviceManager>,
    device_id: String,
) -> Result<Vec<FeatureInfo>> {
    let raw = manager.enumerate_features(&device_id)?;
    Ok(raw
        .into_iter()
        .map(|(id, index, kind)| FeatureInfo {
            id: format!("{id:#06x}"),
            name: feature_name(id).to_string(),
            index,
            obsolete: kind & 0x80 != 0,
            hidden: kind & 0x40 != 0,
        })
        .collect())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureInfo {
    pub id: String,
    pub name: String,
    pub index: u8,
    pub obsolete: bool,
    pub hidden: bool,
}

fn feature_name(id: u16) -> &'static str {
    match id {
        0x0000 => "Root",
        0x0001 => "Feature Set",
        0x0003 => "Device Information",
        0x0005 => "Device Name & Type",
        0x0020 => "Config Change",
        0x1000 => "Battery Level Status",
        0x1001 => "Battery Voltage",
        0x1004 => "Unified Battery",
        0x1b04 => "Reprogrammable Keys",
        0x2201 => "Adjustable DPI",
        0x2205 => "Pointer Motion Scaling",
        0x8060 => "Adjustable Report Rate",
        0x8061 => "Extended Report Rate",
        0x8070 => "Color LED Effects",
        0x8071 => "RGB Effects",
        0x8100 => "Onboard Profiles",
        0x8110 => "Mouse Button Spy",
        _ => "Unknown",
    }
}

/// Sets the sensor DPI. The value is clamped and snapped to what the sensor
/// accepts, and the *applied* value is returned so the slider can correct itself.
#[tauri::command]
pub async fn set_device_dpi(
    app: AppHandle,
    manager: State<'_, DeviceManager>,
    device_id: String,
    dpi: u16,
) -> Result<DpiState> {
    let state = manager.set_dpi(&device_id, dpi)?;
    emit_device_update(&app, &manager, &device_id);
    Ok(state)
}

/// Sets the USB polling rate in Hz (125 / 250 / 500 / 1000, plus 2–8 kHz on
/// devices exposing feature `0x8061`). Returns the rate actually applied.
#[tauri::command]
pub async fn set_polling_rate(
    app: AppHandle,
    manager: State<'_, DeviceManager>,
    device_id: String,
    rate_hz: u32,
) -> Result<ReportRateState> {
    let state = manager.set_report_rate(&device_id, rate_hz)?;
    emit_device_update(&app, &manager, &device_id);
    Ok(state)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingRequest {
    pub device_id: String,
    /// `0xff` targets every zone at once.
    #[serde(default = "all_zones")]
    pub zone: u8,
    /// `#rrggbb`
    pub color: String,
    /// `off` | `fixed` | `breathing` | `cycle`
    pub effect: String,
    #[serde(default = "full_brightness")]
    pub brightness: u8,
    #[serde(default = "default_rate")]
    pub rate_ms: u16,
    /// Write to the device's flash as well as RAM, so it survives a replug.
    #[serde(default = "default_persist")]
    pub persist: bool,
}

fn all_zones() -> u8 {
    0xff
}
fn full_brightness() -> u8 {
    100
}
fn default_rate() -> u16 {
    5000
}
fn default_persist() -> bool {
    true
}

/// Applies a lighting effect. Experimental: the `0x8070` effect parameter layout
/// varies by device generation, so unsupported combinations surface as a
/// protocol error rather than silently doing nothing.
#[tauri::command]
pub async fn set_device_lighting(
    manager: State<'_, DeviceManager>,
    request: LightingRequest,
) -> Result<()> {
    let rgb = parse_hex_color(&request.color)?;
    let effect = match request.effect.as_str() {
        "off" => LightEffect::Off,
        "fixed" => LightEffect::Fixed,
        "breathing" => LightEffect::Breathing {
            rate_ms: request.rate_ms,
            brightness: request.brightness,
        },
        "cycle" => LightEffect::Cycle {
            rate_ms: request.rate_ms,
            brightness: request.brightness,
        },
        other => return Err(Error::other(format!("unknown lighting effect '{other}'"))),
    };
    manager.set_lighting(&request.device_id, request.zone, rgb, effect, request.persist)
}

/// Every lighting zone: index, location name and the effects it accepts.
#[tauri::command]
pub async fn get_lighting_zones(
    manager: State<'_, DeviceManager>,
    device_id: String,
) -> Result<Vec<crate::hidpp::features::ZoneInfo>> {
    manager.lighting_zones(&device_id)
}

fn parse_hex_color(s: &str) -> Result<[u8; 3]> {
    let hex = s.trim_start_matches('#');
    if hex.len() != 6 {
        return Err(Error::other(format!("'{s}' is not a #rrggbb colour")));
    }
    let byte = |i: usize| {
        u8::from_str_radix(&hex[i..i + 2], 16)
            .map_err(|_| Error::other(format!("'{s}' is not a #rrggbb colour")))
    };
    Ok([byte(0)?, byte(2)?, byte(4)?])
}

/// Reads battery for every battery-backed device right now, without waiting for
/// the poller's next tick.
#[tauri::command]
pub async fn read_batteries(
    app: AppHandle,
    manager: State<'_, DeviceManager>,
) -> Result<Vec<BatteryEvent>> {
    let events: Vec<BatteryEvent> = manager
        .poll_batteries()
        .into_iter()
        .map(|(device_id, battery)| BatteryEvent { device_id, battery })
        .collect();
    for event in &events {
        let _ = app.emit(EVENT_BATTERY, event);
    }
    Ok(events)
}

/// Toggles the synthetic catalogue. Handy for UI work on a machine with no gear.
#[tauri::command]
pub async fn set_demo_mode(
    app: AppHandle,
    manager: State<'_, DeviceManager>,
    enabled: bool,
) -> Result<DeviceListPayload> {
    let devices = manager.set_demo(enabled);
    let payload = DeviceListPayload::build(&manager, devices);
    let _ = app.emit(EVENT_DEVICES, &payload);
    Ok(payload)
}

fn emit_device_update(app: &AppHandle, manager: &DeviceManager, device_id: &str) {
    if let Some(snapshot) = manager.snapshot(device_id) {
        let _ = app.emit(EVENT_DEVICE_UPDATED, snapshot);
    }
}

// ---------------------------------------------------------------------------
// Profiles & settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_config(store: State<'_, Store>) -> Result<Config> {
    Ok(store.get())
}

#[tauri::command]
pub async fn save_config(store: State<'_, Store>, config: Config) -> Result<Config> {
    store.replace(config)
}

#[tauri::command]
pub async fn set_active_profile(store: State<'_, Store>, profile_id: String) -> Result<Config> {
    store.update(|cfg| {
        if cfg.profiles.iter().any(|p| p.id == profile_id) {
            cfg.active_profile = profile_id;
        }
    })?;
    Ok(store.get())
}

#[tauri::command]
pub async fn create_profile(
    store: State<'_, Store>,
    name: String,
    kind: Option<String>,
) -> Result<Config> {
    let id = format!(
        "p{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    store.update(|cfg| {
        cfg.profiles.push(Profile {
            id: id.clone(),
            name,
            kind: kind.unwrap_or_else(|| "game".into()),
            devices: Default::default(),
        });
        cfg.active_profile = id.clone();
    })?;
    Ok(store.get())
}

#[tauri::command]
pub async fn delete_profile(store: State<'_, Store>, profile_id: String) -> Result<Config> {
    if profile_id == "default" {
        return Err(Error::other("the default profile cannot be deleted"));
    }
    store.update(|cfg| {
        cfg.profiles.retain(|p| p.id != profile_id);
        if cfg.active_profile == profile_id {
            cfg.active_profile = "default".into();
        }
    })?;
    Ok(store.get())
}

/// Stores the per-device settings of the active profile.
#[tauri::command]
pub async fn save_device_profile(
    store: State<'_, Store>,
    device_id: String,
    profile: DeviceProfile,
) -> Result<Config> {
    store.update(|cfg| {
        let active = cfg.active_profile.clone();
        if let Some(p) = cfg.profiles.iter_mut().find(|p| p.id == active) {
            p.devices.insert(device_id, profile);
        }
    })?;
    Ok(store.get())
}

#[tauri::command]
pub async fn get_device_profile(
    store: State<'_, Store>,
    device_id: String,
) -> Result<DeviceProfile> {
    Ok(store.device_profile(&device_id))
}

#[tauri::command]
pub async fn save_settings(store: State<'_, Store>, settings: Settings) -> Result<Config> {
    store.update(|cfg| cfg.settings = settings)?;
    Ok(store.get())
}

/// Where the config lives, shown on the settings screen.
#[tauri::command]
pub async fn get_config_path(store: State<'_, Store>) -> Result<String> {
    Ok(store.path().display().to_string())
}

// ---------------------------------------------------------------------------
// Onboard profiles
// ---------------------------------------------------------------------------

/// Dumps all onboard memory to a file and returns its path.
#[tauri::command]
pub async fn backup_onboard_memory(
    manager: State<'_, DeviceManager>,
    device_id: String,
) -> Result<String> {
    Ok(manager.backup_onboard(&device_id)?.display().to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardProfiles {
    pub info: crate::hidpp::onboard::OnboardInfo,
    pub profiles: Vec<crate::hidpp::onboard::Profile>,
}

/// Reads and decodes the device's onboard profiles.
#[tauri::command]
pub async fn get_onboard_profiles(
    manager: State<'_, DeviceManager>,
    device_id: String,
) -> Result<OnboardProfiles> {
    let (info, profiles) = manager.read_onboard_profiles(&device_id)?;
    Ok(OnboardProfiles { info, profiles })
}

/// Writes the profile's macro bindings into the device's onboard memory.
///
/// Takes a backup first — this modifies the live profile, and a bad write would
/// otherwise be unrecoverable.
#[tauri::command]
pub async fn apply_onboard_macros(
    manager: State<'_, DeviceManager>,
    device_id: String,
    assignments: Vec<crate::hidpp::onboard::MacroAssignment>,
) -> Result<String> {
    let backup = manager.backup_onboard(&device_id)?;
    manager.apply_onboard_macros(&device_id, assignments)?;
    Ok(backup.display().to_string())
}

/// Restores onboard memory from a backup file.
#[tauri::command]
pub async fn restore_onboard_memory(
    manager: State<'_, DeviceManager>,
    device_id: String,
    path: String,
) -> Result<usize> {
    manager.restore_onboard(&device_id, std::path::Path::new(&path))
}

// ---------------------------------------------------------------------------
// Device artwork
// ---------------------------------------------------------------------------

/// Absolute paths of every artwork file the user has supplied, keyed by product
/// id (`"c08d"`). The frontend turns these into asset URLs.
#[tauri::command]
pub async fn get_artwork() -> Result<std::collections::HashMap<String, String>> {
    Ok(crate::artwork::scan()
        .into_iter()
        .map(|(key, path)| (key, path.display().to_string()))
        .collect())
}

/// Where to put artwork files; shown on the settings screen.
#[tauri::command]
pub async fn get_artwork_dir() -> Result<String> {
    let path = crate::artwork::ensure_dir()
        .map_err(|e| Error::other(format!("could not create the artwork directory: {e}")))?;
    Ok(path.display().to_string())
}

// ---------------------------------------------------------------------------
// Window controls (the title bar is drawn by the frontend)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn window_minimize(app: AppHandle) -> Result<()> {
    if let Some(w) = app.get_webview_window("main") {
        w.minimize().map_err(|e| Error::other(e.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn window_toggle_maximize(app: AppHandle) -> Result<bool> {
    let Some(w) = app.get_webview_window("main") else {
        return Ok(false);
    };
    let maximized = w.is_maximized().map_err(|e| Error::other(e.to_string()))?;
    if maximized {
        w.unmaximize().map_err(|e| Error::other(e.to_string()))?;
    } else {
        w.maximize().map_err(|e| Error::other(e.to_string()))?;
    }
    Ok(!maximized)
}

/// Closing hides to the tray, matching G HUB's behaviour.
#[tauri::command]
pub async fn window_close(app: AppHandle) -> Result<()> {
    if let Some(w) = app.get_webview_window("main") {
        w.hide().map_err(|e| Error::other(e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_colours() {
        assert_eq!(parse_hex_color("#00b8fc").unwrap(), [0x00, 0xb8, 0xfc]);
        assert_eq!(parse_hex_color("FF0000").unwrap(), [0xff, 0x00, 0x00]);
        assert!(parse_hex_color("#fff").is_err());
        assert!(parse_hex_color("#gggggg").is_err());
    }

    #[test]
    fn known_features_are_named() {
        assert_eq!(feature_name(0x2201), "Adjustable DPI");
        assert_eq!(feature_name(0x1234), "Unknown");
    }
}
