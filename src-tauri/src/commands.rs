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
/// Emitted with the full config whenever the backend changes it itself.
pub const EVENT_CONFIG_CHANGED: &str = "config-changed";

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

// ---------------------------------------------------------------------------
// Device permissions (udev rule)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_udev_rule_status() -> Result<crate::permissions::UdevRuleStatus> {
    Ok(crate::permissions::status())
}

/// Installs this build's udev rule through polkit (the desktop asks for the
/// password), then rescans so devices that were root-only come up.
#[tauri::command]
pub async fn install_udev_rule(app: AppHandle, manager: State<'_, DeviceManager>) -> Result<crate::permissions::UdevRuleStatus> {
    let status = tauri::async_runtime::spawn_blocking(crate::permissions::install)
        .await
        .map_err(|e| Error::other(e.to_string()))??;
    let devices = manager.refresh();
    let payload = DeviceListPayload::build(&manager, devices);
    let _ = app.emit(EVENT_DEVICES, &payload);
    crate::refresh_tray_menu(&app);
    crate::apply_all_profiles(&app);
    Ok(status)
}

// ---------------------------------------------------------------------------
// Firmware updates
// ---------------------------------------------------------------------------

/// Emitted while an update runs: `{ deviceId, stage, percent, done, error }`.
pub const EVENT_FIRMWARE_PROGRESS: &str = "firmware-progress";

/// What the catalogue has for a device, against what it runs.
#[tauri::command]
pub async fn check_firmware(manager: State<'_, DeviceManager>, device_id: String) -> Result<crate::firmware::FirmwareCheck> {
    let snap = manager.snapshot(&device_id).ok_or(Error::NotConnected)?;
    Ok(crate::firmware::check(&snap))
}

/// Downloads every firmware package the depository lists (cached after the
/// first time) and returns the check for one device.
#[tauri::command]
pub async fn refresh_firmware_catalog(manager: State<'_, DeviceManager>, device_id: String) -> Result<crate::firmware::FirmwareCheck> {
    tauri::async_runtime::spawn_blocking(crate::firmware::refresh_catalog)
        .await
        .map_err(|e| Error::other(e.to_string()))??;
    let snap = manager.snapshot(&device_id).ok_or(Error::NotConnected)?;
    Ok(crate::firmware::check(&snap))
}

/// Flashes the catalogued package onto the device, reporting progress as
/// `firmware-progress` events, then rescans.
#[tauri::command]
pub async fn update_firmware(app: AppHandle, manager: State<'_, DeviceManager>, device_id: String) -> Result<Vec<crate::hidpp::features::FirmwareInfo>> {
    let snap = manager.snapshot(&device_id).ok_or(Error::NotConnected)?;
    let check = crate::firmware::check(&snap);
    let package = check.package.clone().ok_or_else(|| Error::other("no firmware package for this device"))?;
    if !check.blockers.is_empty() {
        return Err(Error::other(match check.blockers[0].as_str() {
            "BLOCKER_CONNECT_USB" => "connect the device with its USB cable first".to_string(),
            other => format!("update blocked: {other}"),
        }));
    }
    // Software features must let go of the device first.
    app.state::<std::sync::Arc<crate::lightsync::LightSync>>().stop_all();

    let app2 = app.clone();
    let id = device_id.clone();
    let manager_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let manager = manager_app.state::<DeviceManager>();
        manager.update_firmware(&id, &package, |stage, percent| {
            let _ = app2.emit(
                EVENT_FIRMWARE_PROGRESS,
                crate::firmware::Progress { device_id: id.clone(), stage: stage.into(), percent, done: false, error: None },
            );
        })
    })
    .await
    .map_err(|e| Error::other(e.to_string()))?;

    let devices = manager.refresh();
    let payload = DeviceListPayload::build(&manager, devices);
    let _ = app.emit(EVENT_DEVICES, &payload);
    crate::refresh_tray_menu(&app);
    crate::apply_all_profiles(&app);
    match result {
        Ok(()) => {
            let fw = manager.snapshot(&device_id).map(|s| s.firmware).unwrap_or_default();
            let _ = app.emit(
                EVENT_FIRMWARE_PROGRESS,
                crate::firmware::Progress { device_id, stage: "Done".into(), percent: 100, done: true, error: None },
            );
            Ok(fw)
        }
        Err(e) => {
            let _ = app.emit(
                EVENT_FIRMWARE_PROGRESS,
                crate::firmware::Progress { device_id, stage: "Failed".into(), percent: 0, done: true, error: Some(e.to_string()) },
            );
            Err(e)
        }
    }
}

// ---------------------------------------------------------------------------
// Launch at startup
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_autostart() -> Result<bool> {
    Ok(crate::autostart::is_enabled())
}

/// Writes or removes the `~/.config/autostart` entry; returns the new state.
#[tauri::command]
pub async fn set_autostart(on: bool) -> Result<bool> {
    crate::autostart::set_enabled(on)
}

/// Mirrors frontend errors into the backend log, so a bug report from the
/// packaged app (which has no reachable devtools) carries the actual failure.
#[tauri::command]
pub fn frontend_log(level: String, message: String) {
    match level.as_str() {
        "error" => log::error!(target: "frontend", "{message}"),
        "warn" => log::warn!(target: "frontend", "{message}"),
        _ => log::info!(target: "frontend", "{message}"),
    }
}

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
    crate::refresh_tray_menu(&app);
    crate::apply_all_profiles(&app);
    // Newly connected devices get their render fetched in the background.
    crate::spawn_artwork_fetch(app, payload.devices.clone());
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
    store: State<'_, Store>,
) -> Result<DpiState> {
    if manager.snapshot(&device_id).and_then(|s| s.onboard_mode) == Some(true) {
        // Onboard mode: the ladder lives in the profile sector.
        let cfg = store.get();
        let dp = cfg
            .profiles
            .iter()
            .find(|p| p.id == cfg.active_profile)
            .and_then(|p| p.devices.get(&device_id))
            .cloned()
            .unwrap_or_default();
        let mut stages = dp.dpi_stages.clone();
        if stages.is_empty() {
            stages = vec![dpi];
        }
        let active = match stages.iter().position(|s| *s == dpi) {
            Some(i) => i,
            None => {
                let i = dp.active_stage.min(stages.len() - 1);
                stages[i] = dpi;
                i
            }
        };
        manager.write_onboard_dpi(&device_id, &stages, active, dp.shift_stage, dp.report_rate_hz)?;
        emit_device_update(&app, &manager, &device_id);
        return manager
            .snapshot(&device_id)
            .and_then(|s| s.dpi)
            .ok_or_else(|| Error::other("no DPI state"));
    }
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
    store: State<'_, Store>,
) -> Result<ReportRateState> {
    if manager.snapshot(&device_id).and_then(|s| s.onboard_mode) == Some(true) {
        let cfg = store.get();
        let dp = cfg
            .profiles
            .iter()
            .find(|p| p.id == cfg.active_profile)
            .and_then(|p| p.devices.get(&device_id))
            .cloned()
            .unwrap_or_default();
        let stages = if dp.dpi_stages.is_empty() { vec![800] } else { dp.dpi_stages.clone() };
        manager.write_onboard_dpi(&device_id, &stages, dp.active_stage, dp.shift_stage, Some(rate_hz))?;
        let _ = manager.refresh_device(&device_id);
        emit_device_update(&app, &manager, &device_id);
        return manager
            .snapshot(&device_id)
            .and_then(|s| s.report_rate)
            .ok_or_else(|| Error::other("no report rate state"));
    }
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
    // 0x8071 devices take live RGB writes even in onboard mode, so they never
    // need the onboard LED table — which is flash, rewritten on every colour
    // picker drag.
    let rgb_effects = manager
        .with_handle(&request.device_id, |h| Ok(h.supports(crate::hidpp::features::lighting::ID_RGB_EFFECTS)))
        .unwrap_or(false);
    if !rgb_effects
        && manager.snapshot(&request.device_id).and_then(|s| s.onboard_mode) == Some(true)
        && request.persist
    {
        // Onboard mode: 0x8070 writes are ignored, the profile's LED table is
        // what the device shows.
        let zones: Vec<u8> = if request.zone == all_zones() {
            (0..manager.snapshot(&request.device_id).map(|s| s.lighting_zones).unwrap_or(1).max(1)).collect()
        } else {
            vec![request.zone]
        };
        let entries: Vec<(u8, [u8; 3], LightEffect)> = zones.into_iter().map(|z| (z, rgb, effect)).collect();
        return manager.write_onboard_lighting(&request.device_id, &entries);
    }
    let mut result = manager.set_lighting(&request.device_id, request.zone, rgb, effect, request.persist);
    // The failed call has already rescanned; a colour is safe to send twice.
    if matches!(&result, Err(e) if crate::state::connection_lost(e)) {
        result = manager.set_lighting(&request.device_id, request.zone, rgb, effect, request.persist);
    }
    if let Err(e) = &result {
        log::warn!("{}: lighting zone {} not applied: {e}", request.device_id, request.zone);
    }
    result
}

/// Switches a device in or out of on-board memory mode and remembers it.
/// Going onboard writes the active profile (DPI, lighting, assignments) into
/// the device so it behaves the same with OpenGHub closed.
#[tauri::command]
pub async fn set_onboard_mode(
    app: AppHandle,
    manager: State<'_, DeviceManager>,
    store: State<'_, Store>,
    device_id: String,
    on: bool,
) -> Result<DeviceSnapshot> {
    manager.set_onboard_mode(&device_id, on)?;
    let id = device_id.clone();
    store.update(move |cfg| {
        cfg.settings.onboard_mode_devices.retain(|d| *d != id);
        if on {
            cfg.settings.onboard_mode_devices.push(id);
        }
    })?;
    if on {
        crate::write_profile_to_device(&app, &device_id);
    }
    crate::apply_assignment_profiles(&app);
    let _ = manager.refresh_device(&device_id);
    emit_device_update(&app, &manager, &device_id);
    let _ = app.emit(EVENT_CONFIG_CHANGED, store.get());
    manager.snapshot(&device_id).ok_or(Error::NotConnected)
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
    crate::refresh_tray_menu(&app);
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
pub async fn set_active_profile(app: AppHandle, store: State<'_, Store>, profile_id: String) -> Result<Config> {
    store.update(|cfg| {
        if let Some(p) = cfg.profiles.iter().find(|p| p.id == profile_id) {
            // Remember which of a game's profiles the user wants when it runs.
            if let Some(app_id) = &p.application_id {
                cfg.settings.active_profile_per_app.insert(app_id.clone(), p.id.clone());
            }
            cfg.active_profile = profile_id;
        }
    })?;
    crate::apply_all_profiles(&app);
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
            application_id: None,
            poster_url: None,
            disabled: false,
            devices: Default::default(),
            script: None,
        });
        cfg.active_profile = id.clone();
    })?;
    Ok(store.get())
}

/// Renames a profile. Game profiles keep their "Game: " prefix unless the new
/// name carries its own.
#[tauri::command]
pub async fn rename_profile(store: State<'_, Store>, profile_id: String, name: String) -> Result<Config> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(Error::other("the name cannot be empty"));
    }
    store.update(|cfg| {
        if let Some(p) = cfg.profiles.iter_mut().find(|p| p.id == profile_id) {
            p.name = match (p.name.split_once(':'), name.contains(':')) {
                (Some((game, _)), false) if p.id != "default" => format!("{}: {name}", game.trim()),
                _ => name.clone(),
            };
        }
    })?;
    Ok(store.get())
}

/// Copies a profile — devices, assignments, macros, lighting — under a new
/// id, bound to the same game, and makes it active.
#[tauri::command]
pub async fn duplicate_profile(app: AppHandle, store: State<'_, Store>, profile_id: String) -> Result<Config> {
    let id = format!(
        "p{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    store.update(|cfg| {
        let Some(src) = cfg.profiles.iter().find(|p| p.id == profile_id).cloned() else { return };
        let name = match src.name.split_once(':') {
            Some((game, rest)) if src.id != "default" => format!("{}: {} Copy", game.trim(), rest.trim()),
            _ => format!("{} Copy", src.name.replace("Desktop: ", "")),
        };
        let name = if src.id == "default" { format!("Desktop: {}", name.trim_start_matches("Desktop: ")) } else { name };
        cfg.profiles.push(Profile {
            id: id.clone(),
            name,
            kind: if src.id == "default" { "game".into() } else { src.kind.clone() },
            application_id: src.application_id.clone(),
            poster_url: src.poster_url.clone(),
            disabled: false,
            devices: src.devices.clone(),
            script: src.script.clone(),
        });
        cfg.active_profile = id.clone();
        if let Some(app_id) = &src.application_id {
            cfg.settings.active_profile_per_app.insert(app_id.clone(), id.clone());
        }
    })?;
    crate::apply_all_profiles(&app);
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
// Application database (game detection, per-game commands)
// ---------------------------------------------------------------------------

/// Emitted when the detected running game changes; payload is the application
/// id, or `null` when no known game is running.
pub const EVENT_ACTIVE_APPLICATION: &str = "active-application";

#[tauri::command]
pub async fn get_applications(db: State<'_, std::sync::Arc<crate::apps::AppDatabase>>) -> Result<Vec<crate::apps::Application>> {
    Ok(db.applications())
}

#[tauri::command]
pub async fn get_application_commands(
    db: State<'_, std::sync::Arc<crate::apps::AppDatabase>>,
    application_id: String,
) -> Result<crate::apps::ApplicationCommands> {
    db.commands(&application_id)
        .ok_or_else(|| Error::other(format!("unknown application {application_id}")))
}

#[tauri::command]
pub async fn get_application_database_info(
    db: State<'_, std::sync::Arc<crate::apps::AppDatabase>>,
) -> Result<Option<crate::apps::DatabaseInfo>> {
    Ok(db.info())
}

/// Re-downloads the database from Logitech's public channel.
#[tauri::command]
pub async fn refresh_application_database(
    db: State<'_, std::sync::Arc<crate::apps::AppDatabase>>,
) -> Result<crate::apps::DatabaseInfo> {
    let db = std::sync::Arc::clone(db.inner());
    tauri::async_runtime::spawn_blocking(move || db.refresh())
        .await
        .map_err(|e| Error::other(e.to_string()))?
}

/// The application detected as running right now.
#[tauri::command]
pub async fn get_active_application(
    db: State<'_, std::sync::Arc<crate::apps::AppDatabase>>,
) -> Result<Option<String>> {
    Ok(db.detect_running())
}

/// Disables or re-enables a game's profile without deleting it.
#[tauri::command]
pub async fn set_profile_disabled(
    store: State<'_, Store>,
    profile_id: String,
    disabled: bool,
) -> Result<Config> {
    store.update(|cfg| {
        if let Some(p) = cfg.profiles.iter_mut().find(|p| p.id == profile_id) {
            p.disabled = disabled;
        }
    })?;
    Ok(store.get())
}

/// Binds a profile to an application so it activates when that game runs.
#[tauri::command]
pub async fn bind_profile_application(
    store: State<'_, Store>,
    db: State<'_, std::sync::Arc<crate::apps::AppDatabase>>,
    profile_id: String,
    application_id: Option<String>,
) -> Result<Config> {
    let app = application_id
        .as_deref()
        .and_then(|id| db.applications().into_iter().find(|a| a.id == id));
    store.update(|cfg| {
        if let Some(p) = cfg.profiles.iter_mut().find(|p| p.id == profile_id) {
            p.application_id = application_id.clone();
            p.poster_url = app.as_ref().and_then(|a| a.poster_url.clone());
            if let Some(a) = &app {
                p.kind = "game".into();
                // Match G HUB's "GAME: Profile" naming unless the user renamed it.
                if !p.name.contains(':') {
                    p.name = format!("{}: {}", a.name, p.name);
                }
            }
        }
    })?;
    Ok(store.get())
}

// ---------------------------------------------------------------------------
// Per-device settings (power, low battery, button layout)
// ---------------------------------------------------------------------------

use crate::profiles::DeviceSettings;

#[tauri::command]
pub async fn get_device_settings(store: State<'_, Store>, device_id: String) -> Result<DeviceSettings> {
    Ok(store.get().settings.device_settings.get(&device_id).cloned().unwrap_or_default())
}

/// Stores the device settings and pushes them: timeouts into the onboard
/// profile, the button layout through the assignment plan, low-battery mode
/// to the battery poller.
#[tauri::command]
pub async fn set_device_settings(
    app: AppHandle,
    store: State<'_, Store>,
    device_id: String,
    settings: DeviceSettings,
) -> Result<DeviceSettings> {
    let id = device_id.clone();
    let s = settings.clone();
    store.update(move |cfg| {
        cfg.settings.device_settings.insert(id, s);
    })?;
    crate::apply_device_settings(&app, Some(&device_id));
    crate::apply_assignment_profiles(&app);
    let _ = app.emit(EVENT_CONFIG_CHANGED, store.get());
    Ok(settings)
}

// ---------------------------------------------------------------------------
// G HUB settings.db import
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhubImportSummary {
    pub profiles: Vec<String>,
    pub devices: Vec<String>,
    pub skipped: Vec<String>,
}

/// Imports profiles from a G HUB `settings.db`: DPI table and shift, report
/// rate, per-zone lighting and button assignments, for every connected device
/// the file has settings for. Game profiles are bound to the same Logitech
/// application ids our database uses.
#[tauri::command]
pub async fn import_ghub_settings(
    app: AppHandle,
    manager: State<'_, DeviceManager>,
    store: State<'_, Store>,
    db: State<'_, std::sync::Arc<crate::apps::AppDatabase>>,
    path: String,
) -> Result<GhubImportSummary> {
    let doc = tauri::async_runtime::spawn_blocking(move || crate::ghub_settings::read_document(std::path::Path::new(&path)))
        .await
        .map_err(|e| Error::other(e.to_string()))??;
    let report = crate::ghub_settings::parse(&doc);

    // Slot prefix (`g502wireless`) → connected device ids, via the device
    // database's model ids and product ids.
    let mut defs = crate::depot::load_cache().map(|(_, d)| d).unwrap_or_default();
    defs.extend(crate::depot::builtin_defs());
    let snapshots = manager.snapshots();
    let device_for_prefix = |prefix: &str| -> Vec<String> {
        let pids: Vec<u16> = defs
            .iter()
            .filter(|d| d.model_id.replace(['_', '-'], "") == prefix)
            .flat_map(|d| d.product_ids.clone())
            .collect();
        snapshots
            .iter()
            .filter(|s| !s.demo && (pids.contains(&s.product_id) || s.model_ids.iter().any(|m| pids.contains(m))))
            .map(|s| s.id.clone())
            .collect()
    };
    let apps = db.applications();

    let mut summary = GhubImportSummary { profiles: vec![], devices: vec![], skipped: vec![] };
    for imported in &report.profiles {
        summary.skipped.extend(imported.skipped.iter().cloned());
        let profile_id = store.update(|cfg| {
            let id = match &imported.application_id {
                None => "default".to_string(),
                Some(app_id) => match cfg.profiles.iter().find(|p| p.application_id.as_deref() == Some(app_id)) {
                    Some(p) => p.id.clone(),
                    None => {
                        let game = apps.iter().find(|a| &a.id == app_id);
                        let name = match game {
                            Some(g) => format!("{}: {}", g.name, imported.name),
                            None => imported.name.clone(),
                        };
                        let id = format!("g{}", imported.ghub_id.chars().take(8).collect::<String>());
                        cfg.profiles.push(Profile {
                            id: id.clone(),
                            name,
                            kind: "game".into(),
                            application_id: Some(app_id.clone()),
                            poster_url: game.and_then(|g| g.poster_url.clone()),
                            disabled: false,
                            devices: Default::default(),
                            script: None,
                        });
                        id
                    }
                },
            };
            id
        })?;
        summary.profiles.push(imported.name.clone());

        for (prefix, dp) in &imported.devices {
            let ids = device_for_prefix(prefix);
            if ids.is_empty() {
                summary.skipped.push(format!("{prefix}: no connected device matches"));
                continue;
            }
            for device_id in ids {
                let dp = dp.clone();
                let did = device_id.clone();
                let pid = profile_id.clone();
                store.update(move |cfg| {
                    if let Some(p) = cfg.profiles.iter_mut().find(|p| p.id == pid) {
                        let entry = p.devices.entry(did).or_default();
                        if !dp.dpi_stages.is_empty() {
                            entry.dpi_stages = dp.dpi_stages.clone();
                            entry.active_stage = dp.active_stage;
                            entry.shift_stage = dp.shift_stage;
                        }
                        if dp.report_rate_hz.is_some() {
                            entry.report_rate_hz = dp.report_rate_hz;
                        }
                        if !dp.lighting_zones.is_empty() {
                            entry.lighting_zones = dp.lighting_zones.clone();
                            entry.lighting = dp.lighting.clone();
                        }
                        if !dp.assignments.is_empty() {
                            entry.assignments = dp.assignments.clone();
                        }
                    }
                })?;
                if !summary.devices.contains(&device_id) {
                    summary.devices.push(device_id);
                }
            }
        }
    }

    let _ = app.emit(EVENT_CONFIG_CHANGED, store.get());
    crate::apply_all_profiles(&app);
    Ok(summary)
}

// ---------------------------------------------------------------------------
// Software lighting (screen sampler / audio visualizer)
// ---------------------------------------------------------------------------

use crate::lightsync::{LightSync, SoftwareEffect};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightSyncStatus {
    pub active_zones: usize,
    pub error: Option<String>,
    pub screen_authorised: bool,
}

/// Starts (or clears) a software effect on one zone. The Lighting screen
/// calls this instead of `set_device_lighting` for screen / audio effects;
/// the sources start on the next engine tick (the screen one may show the
/// desktop's sharing dialog once).
#[tauri::command]
pub async fn set_zone_software_effect(
    sync: State<'_, std::sync::Arc<LightSync>>,
    store: State<'_, Store>,
    device_id: String,
    zone: u8,
    effect: Option<SoftwareEffect>,
) -> Result<()> {
    sync.set_zone((device_id, zone), effect);
    // Remember a restore token as soon as we have one.
    if let Some(t) = sync.restore_token.lock().clone() {
        if store.get().settings.screen_restore_token.as_deref() != Some(t.as_str()) {
            let _ = store.update(|c| c.settings.screen_restore_token = Some(t));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_lightsync_status(
    sync: State<'_, std::sync::Arc<LightSync>>,
    store: State<'_, Store>,
) -> Result<LightSyncStatus> {
    if let Some(t) = sync.restore_token.lock().clone() {
        if store.get().settings.screen_restore_token.as_deref() != Some(t.as_str()) {
            let _ = store.update(|c| c.settings.screen_restore_token = Some(t));
        }
    }
    Ok(LightSyncStatus {
        active_zones: sync.effects().len(),
        error: sync.last_error.lock().clone(),
        screen_authorised: sync.restore_token.lock().is_some(),
    })
}

// ---------------------------------------------------------------------------
// Assignments
// ---------------------------------------------------------------------------

/// Pushes the active profile's assignments for one device to the hardware:
/// software mode (spy + remapping) now, and the onboard table where there is
/// one. Called by the Assignments screen after every change.
#[tauri::command]
pub async fn apply_assignments(
    app: AppHandle,
    manager: State<'_, DeviceManager>,
    store: State<'_, Store>,
    device_id: String,
) -> Result<crate::state::AssignmentReport> {
    let cfg = store.get();
    let dp = cfg
        .profiles
        .iter()
        .find(|p| p.id == cfg.active_profile)
        .and_then(|p| p.devices.get(&device_id))
        .cloned()
        .unwrap_or_default();
    let script_active = cfg
        .profiles
        .iter()
        .find(|p| p.id == cfg.active_profile)
        .and_then(|p| p.script.as_deref())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let report = manager.apply_assignments(&device_id, &dp.assignments, &dp.macros, script_active)?;
    emit_device_update(&app, &manager, &device_id);
    Ok(report)
}

// ---------------------------------------------------------------------------
// Lua scripting
// ---------------------------------------------------------------------------

use crate::scripting::Scripting;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptStatus {
    pub running: bool,
    /// Profile whose script is running, if any.
    pub profile_id: Option<String>,
    /// Devices in on-board memory mode: their buttons never reach the host,
    /// so the script does not see them.
    pub onboard_devices: Vec<String>,
}

/// Saves a profile's Lua script. When that profile is active the script is
/// (re)started right away — G HUB's "Save & Run"; an empty script stops it.
#[tauri::command]
pub async fn set_profile_script(
    app: AppHandle,
    store: State<'_, Store>,
    scripting: State<'_, std::sync::Arc<Scripting>>,
    profile_id: String,
    script: Option<String>,
) -> Result<Config> {
    let script = script.filter(|s| !s.trim().is_empty());
    store.update(|cfg| {
        if let Some(p) = cfg.profiles.iter_mut().find(|p| p.id == profile_id) {
            p.script = script.clone();
        }
    })?;
    let cfg = store.get();
    if cfg.active_profile == profile_id {
        match &script {
            Some(src) => crate::start_script(&app, &profile_id, src),
            None => scripting.stop(),
        }
        // The spy has to be on for the script to hear buttons, off again once
        // nothing needs it.
        crate::apply_assignment_profiles(&app);
    }
    let _ = app.emit(EVENT_CONFIG_CHANGED, cfg.clone());
    Ok(cfg)
}

#[tauri::command]
pub async fn get_script_log(scripting: State<'_, std::sync::Arc<Scripting>>) -> Result<Vec<String>> {
    Ok(scripting.log_lines())
}

#[tauri::command]
pub async fn clear_script_log(scripting: State<'_, std::sync::Arc<Scripting>>) -> Result<()> {
    scripting.clear_log();
    Ok(())
}

#[tauri::command]
pub async fn get_script_status(
    manager: State<'_, DeviceManager>,
    scripting: State<'_, std::sync::Arc<Scripting>>,
) -> Result<ScriptStatus> {
    let onboard_devices = manager
        .snapshots()
        .into_iter()
        .filter(|s| s.online && s.onboard_mode == Some(true))
        .map(|s| s.name)
        .collect();
    Ok(ScriptStatus { running: scripting.is_running(), profile_id: scripting.active_profile(), onboard_devices })
}

// ---------------------------------------------------------------------------
// Steering wheels
// ---------------------------------------------------------------------------

use crate::wheel::{WheelSettings, WheelState};

/// Emitted ~30× per second while a wheel moves: `{ deviceId, state }`.
pub const EVENT_WHEEL_STATE: &str = "wheel-state";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelStateEvent {
    pub device_id: String,
    pub state: WheelState,
}

#[tauri::command]
pub async fn get_wheel_state(manager: State<'_, DeviceManager>, device_id: String) -> Result<WheelState> {
    manager.wheel_state(&device_id)
}

#[tauri::command]
pub async fn get_wheel_settings(manager: State<'_, DeviceManager>, device_id: String) -> Result<WheelSettings> {
    manager.wheel_settings(&device_id)
}

/// Writes the settings to the wheel and stores them in the active profile.
#[tauri::command]
pub async fn set_wheel_settings(
    app: AppHandle,
    manager: State<'_, DeviceManager>,
    store: State<'_, Store>,
    device_id: String,
    settings: WheelSettings,
) -> Result<WheelSettings> {
    let applied = manager.apply_wheel_settings(&device_id, &settings)?;
    let profile_id = store.get().active_profile;
    store.update(|cfg| {
        if let Some(p) = cfg.profiles.iter_mut().find(|p| p.id == profile_id) {
            let dp = p.devices.entry(device_id.clone()).or_default();
            dp.wheel = Some(applied.clone());
        }
    })?;
    let _ = app.emit(EVENT_CONFIG_CHANGED, store.get());
    emit_device_update(&app, &manager, &device_id);
    Ok(applied)
}

#[tauri::command]
pub async fn set_wheel_leds(manager: State<'_, DeviceManager>, device_id: String, mask: u8) -> Result<()> {
    manager.set_wheel_leds(&device_id, mask)
}

/// G HUB's "Calibrate wheel center position": the current position becomes
/// the centre (software offset, ±`max_degrees`), stored in the active profile.
#[tauri::command]
pub async fn calibrate_wheel_center(
    app: AppHandle,
    manager: State<'_, DeviceManager>,
    store: State<'_, Store>,
    device_id: String,
    max_degrees: Option<f32>,
) -> Result<WheelSettings> {
    manager.calibrate_wheel_center(&device_id, max_degrees.unwrap_or(10.0))?;
    let settings = manager.wheel_settings(&device_id)?;
    let profile_id = store.get().active_profile;
    store.update(|cfg| {
        if let Some(p) = cfg.profiles.iter_mut().find(|p| p.id == profile_id) {
            let dp = p.devices.entry(device_id.clone()).or_default();
            dp.wheel = Some(settings.clone());
        }
    })?;
    let _ = app.emit(EVENT_CONFIG_CHANGED, store.get());
    Ok(settings)
}

/// Clears the software centre offset.
#[tauri::command]
pub async fn reset_wheel_center(
    app: AppHandle,
    manager: State<'_, DeviceManager>,
    store: State<'_, Store>,
    device_id: String,
) -> Result<WheelSettings> {
    let mut settings = manager.wheel_settings(&device_id)?;
    settings.center_offset = 0;
    let applied = manager.apply_wheel_settings(&device_id, &settings)?;
    let profile_id = store.get().active_profile;
    store.update(|cfg| {
        if let Some(p) = cfg.profiles.iter_mut().find(|p| p.id == profile_id) {
            let dp = p.devices.entry(device_id.clone()).or_default();
            dp.wheel = Some(applied.clone());
        }
    })?;
    let _ = app.emit(EVENT_CONFIG_CHANGED, store.get());
    Ok(applied)
}

/// Starts or stops the userspace force-feedback driver and remembers the choice.
#[tauri::command]
pub async fn set_wheel_driver(
    app: AppHandle,
    manager: State<'_, DeviceManager>,
    store: State<'_, Store>,
    enabled: bool,
) -> Result<Config> {
    store.update(|cfg| cfg.settings.wheel_driver = enabled)?;
    for id in manager.wheel_ids() {
        manager.set_wheel_driver(&id, enabled)?;
        emit_device_update(&app, &manager, &id);
    }
    Ok(store.get())
}

// ---------------------------------------------------------------------------
// Games library
// ---------------------------------------------------------------------------

use crate::games::{self, Game, Library};

/// The installed games from every launcher on this machine. Cached after the
/// first scan; `refresh` re-reads the launchers.
#[tauri::command]
pub async fn get_games(
    library: State<'_, Library>,
    store: State<'_, Store>,
    db: State<'_, std::sync::Arc<crate::apps::AppDatabase>>,
    refresh: bool,
) -> Result<Vec<Game>> {
    if !refresh {
        if let Some(games) = library.cached() {
            return Ok(games);
        }
    }
    let manual = store.get().manual_games;
    let db = db.inner().clone();
    let games = tauri::async_runtime::spawn_blocking(move || games::scan(&manual, &db))
        .await
        .map_err(|e| Error::other(e.to_string()))?;
    library.set(games.clone());
    Ok(games)
}

/// Starts a game through its launcher. Nothing else changes: profile switching
/// happens through the usual watcher once the process is up.
#[tauri::command]
pub async fn launch_game(store: State<'_, Store>, game_id: String) -> Result<()> {
    let manual = store.get().manual_games;
    tauri::async_runtime::spawn_blocking(move || games::launch(&game_id, &manual))
        .await
        .map_err(|e| Error::other(e.to_string()))?
}

#[tauri::command]
pub async fn add_manual_game(
    store: State<'_, Store>,
    library: State<'_, Library>,
    name: String,
    exec: String,
    args: Option<String>,
    cover: Option<String>,
) -> Result<Config> {
    let game = games::new_manual_game(&name, &exec, args.as_deref().unwrap_or(""), cover.as_deref())?;
    store.update(|cfg| cfg.manual_games.push(game))?;
    library.invalidate();
    Ok(store.get())
}

#[tauri::command]
pub async fn remove_manual_game(
    store: State<'_, Store>,
    library: State<'_, Library>,
    id: String,
) -> Result<Config> {
    store.update(|cfg| cfg.manual_games.retain(|g| g.id != id))?;
    library.invalidate();
    Ok(store.get())
}

// ---------------------------------------------------------------------------
// Community profiles
// ---------------------------------------------------------------------------

use crate::community::{self, CommunityIndex, SharedProfile};

#[tauri::command]
pub async fn get_community_index(store: State<'_, Store>, refresh: bool) -> Result<CommunityIndex> {
    let repo = store.get().settings.community_repo;
    tauri::async_runtime::spawn_blocking(move || community::fetch_index(&repo, refresh))
        .await
        .map_err(|e| Error::other(e.to_string()))?
}

/// Fetches one shared profile so the UI can show what it contains — DPI,
/// lighting and every macro step — before the user decides to import it.
#[tauri::command]
pub async fn preview_community_profile(
    store: State<'_, Store>,
    path: String,
) -> Result<SharedProfile> {
    let repo = store.get().settings.community_repo;
    tauri::async_runtime::spawn_blocking(move || community::fetch_profile(&repo, &path))
        .await
        .map_err(|e| Error::other(e.to_string()))?
}

/// Imports a shared profile as a new local profile. The settings are applied
/// to every connected device whose product id the profile lists; nothing is
/// written to hardware here — that still happens through the normal screens.
#[tauri::command]
pub async fn import_community_profile(
    store: State<'_, Store>,
    manager: State<'_, DeviceManager>,
    db: State<'_, std::sync::Arc<crate::apps::AppDatabase>>,
    shared: SharedProfile,
) -> Result<Config> {
    community::validate(&shared)?;
    // The game's poster from the application database, as binding does.
    let poster_url = shared
        .application
        .as_ref()
        .and_then(|a| db.applications().into_iter().find(|x| x.id == a.id))
        .and_then(|a| a.poster_url.clone());

    // Which local devices this profile targets.
    let targets: Vec<String> = manager
        .snapshots()
        .into_iter()
        .filter(|d| {
            let mut ids = d.model_ids.clone();
            ids.push(d.product_id);
            ids.iter().any(|id| shared.device.product_ids.contains(id))
        })
        .map(|d| d.id)
        .collect();
    if targets.is_empty() {
        return Err(Error::other(format!(
            "no connected device matches this profile ({}). Connect it and try again.",
            shared.device.display_name.as_str()
        )));
    }

    let id = format!("c{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0));
    store.update(|cfg| {
        let mut devices = std::collections::HashMap::new();
        for target in &targets {
            devices.insert(target.clone(), shared.profile.clone());
        }
        cfg.profiles.push(Profile {
            id: id.clone(),
            name: shared.name.clone(),
            kind: if shared.application.is_some() { "game".into() } else { "app".into() },
            application_id: shared.application.as_ref().map(|a| a.id.clone()),
            poster_url: poster_url.clone(),
            disabled: false,
            devices,
            script: None,
        });
    })?;
    Ok(store.get())
}

/// Builds the shareable JSON for a profile + device, ready to be saved and
/// contributed to the community repository with a pull request.
#[tauri::command]
pub async fn export_profile(
    store: State<'_, Store>,
    manager: State<'_, DeviceManager>,
    profile_id: String,
    device_id: String,
    description: String,
) -> Result<String> {
    let cfg = store.get();
    let profile = cfg
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| Error::other("unknown profile"))?;
    let snapshot = manager.snapshot(&device_id).ok_or(Error::NotConnected)?;

    // Prefer G HUB's model id when the device database is cached; otherwise a
    // name-derived slug is still a stable enough key.
    let model_id = crate::depot::load_cache()
        .ok()
        .and_then(|(_, defs)| {
            defs.into_iter()
                .find(|d| d.product_ids.iter().any(|p| *p == snapshot.product_id || snapshot.model_ids.contains(p)))
                .map(|d| d.model_id)
        })
        .unwrap_or_else(|| community::slugify(&snapshot.name).replace('-', "_"));

    let mut product_ids = snapshot.model_ids.clone();
    if !product_ids.contains(&snapshot.product_id) {
        product_ids.push(snapshot.product_id);
    }
    let device = community::TargetDevice {
        model_id,
        product_ids,
        display_name: snapshot.name.clone(),
        kind: format!("{:?}", snapshot.kind).to_lowercase(),
    };
    let author = if cfg.settings.author_name.trim().is_empty() {
        "anonymous".to_string()
    } else {
        cfg.settings.author_name.clone()
    };
    let shared = community::export(profile, &device_id, device, &author, &description)?;
    serde_json::to_string_pretty(&shared).map_err(|e| Error::other(e.to_string()))
}

/// Writes the exported JSON where the user chose (via the save dialog).
#[tauri::command]
pub async fn write_text_file(path: String, contents: String) -> Result<()> {
    std::fs::write(&path, contents).map_err(|e| Error::other(format!("could not write {path}: {e}")))
}

/// Reads a file the user picked in the open dialog (Lua script import).
#[tauri::command]
pub async fn read_text_file(path: String) -> Result<String> {
    std::fs::read_to_string(&path).map_err(|e| Error::other(format!("could not read {path}: {e}")))
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

/// Imports renders, thumbnails and zone/button layouts from a G HUB
/// `ProgramData\\LGHUB` folder the user already has, and caches its depository
/// so further devices can be fetched on demand.
#[tauri::command]
pub async fn import_ghub_program_data(path: String) -> Result<crate::depot::ImportReport> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::depot::import_program_data(std::path::Path::new(&path))
    })
    .await
    .map_err(|e| Error::other(e.to_string()))?
}

/// Downloads a connected device's depot from Logitech's CDN — what G HUB does
/// on first sight of a device — and imports it. Needs a cached depository.
#[tauri::command]
pub async fn fetch_device_artwork(
    manager: State<'_, DeviceManager>,
    device_id: String,
) -> Result<crate::depot::ImportedDevice> {
    let snapshot = manager.snapshot(&device_id).ok_or(Error::NotConnected)?;
    let mut ids = snapshot.model_ids.clone();
    ids.push(snapshot.product_id);
    tauri::async_runtime::spawn_blocking(move || crate::depot::fetch_for_product_ids(&ids))
        .await
        .map_err(|e| Error::other(e.to_string()))?
}

/// Whether depots can be fetched (a depository has been imported).
#[tauri::command]
pub async fn get_ghub_cache_info() -> Result<Option<GhubCacheInfo>> {
    Ok(crate::depot::load_cache().ok().map(|(d, defs)| GhubCacheInfo {
        build_id: d.build_id,
        version: d.version,
        depots: d.depots.len(),
        device_definitions: defs.len(),
    }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GhubCacheInfo {
    pub build_id: String,
    pub version: String,
    pub depots: usize,
    pub device_definitions: usize,
}

/// The imported zone/button layout for a device, if any.
#[tauri::command]
pub async fn get_artwork_layout(product_ids: Vec<u16>) -> Result<Option<crate::depot::ArtworkLayout>> {
    Ok(crate::artwork::layout_for(&product_ids))
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
