//! OpenGHub — an open source, Linux-native reimplementation of Logitech G HUB.

pub mod apps;
pub mod artwork;
pub mod autostart;
pub mod commands;
pub mod community;
pub mod demo;
pub mod depot;
pub mod firmware;
pub mod games;
pub mod ghub_settings;
pub mod keymap;
pub mod lightsync;
pub mod permissions;
pub mod remap;
pub mod scripting;
pub mod hidpp;
pub mod profiles;
pub mod state;
pub mod uinput;
pub mod wheel;

use std::time::Duration;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, WindowEvent};

use commands::{BatteryEvent, EVENT_BATTERY};
use profiles::Store;
use state::DeviceManager;

/// Lower bound on the battery poll interval. HID++ round trips wake sleeping
/// wireless devices, so polling faster than this measurably costs battery.
const MIN_POLL_SECONDS: u64 = 15;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(DeviceManager::new())
        .manage(Store::load())
        .manage(std::sync::Arc::new(apps::AppDatabase::new()))
        .manage(games::Library::default())
        .manage(std::sync::Arc::new(remap::Injector::new()))
        .manage(std::sync::Arc::new(lightsync::LightSync::new()))
        .manage(std::sync::Arc::new(scripting::Scripting::new()))
        .setup(|app| {
            let handle = app.handle().clone();
            app.manage(ScriptHost(spawn_script_host(handle.clone())));

            // Initial scan so the dashboard has data before the first IPC call.
            let manager = app.state::<DeviceManager>();
            let devices = manager.refresh();
            log::info!("found {} device(s) (demo: {})", devices.len(), manager.is_demo());
            spawn_artwork_fetch(handle.clone(), devices);

            build_tray(app)?;
            // "Start minimised to tray", and always when launched by the
            // autostart entry: the window exists but stays hidden.
            let minimised = autostart::started_minimized() || app.state::<Store>().get().settings.start_minimised;
            if minimised {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            apply_all_profiles(&handle);
            lightsync::spawn(handle.clone());
            spawn_button_pump(handle.clone());
            spawn_wheel_telemetry(handle.clone());
            spawn_battery_poller(handle.clone());
            spawn_application_watcher(handle);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window hides to tray; quitting happens from the tray menu.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_connected_devices,
            commands::get_udev_rule_status,
            commands::install_udev_rule,
            commands::get_autostart,
            commands::set_autostart,
            commands::check_firmware,
            commands::refresh_firmware_catalog,
            commands::update_firmware,
            commands::get_device_state,
            commands::get_device_features,
            commands::set_device_dpi,
            commands::set_polling_rate,
            commands::set_device_lighting,
            commands::get_lighting_zones,
            commands::get_applications,
            commands::get_application_commands,
            commands::get_application_database_info,
            commands::refresh_application_database,
            commands::get_active_application,
            commands::bind_profile_application,
            commands::set_profile_disabled,
            commands::get_community_index,
            commands::preview_community_profile,
            commands::import_community_profile,
            commands::export_profile,
            commands::apply_assignments,
            commands::set_profile_script,
            commands::get_script_log,
            commands::clear_script_log,
            commands::get_script_status,
            commands::import_ghub_settings,
            commands::get_device_settings,
            commands::set_device_settings,
            commands::set_onboard_mode,
            commands::set_zone_software_effect,
            commands::get_lightsync_status,
            commands::get_wheel_state,
            commands::get_wheel_settings,
            commands::set_wheel_settings,
            commands::set_wheel_leds,
            commands::calibrate_wheel_center,
            commands::reset_wheel_center,
            commands::set_wheel_driver,
            commands::get_games,
            commands::launch_game,
            commands::add_manual_game,
            commands::remove_manual_game,
            commands::write_text_file,
            commands::read_text_file,
            commands::backup_onboard_memory,
            commands::get_onboard_profiles,
            commands::apply_onboard_macros,
            commands::restore_onboard_memory,
            commands::import_ghub_program_data,
            commands::fetch_device_artwork,
            commands::get_ghub_cache_info,
            commands::get_artwork_layout,
            commands::get_artwork,
            commands::get_artwork_dir,
            commands::read_batteries,
            commands::set_demo_mode,
            commands::get_config,
            commands::save_config,
            commands::set_active_profile,
            commands::create_profile,
            commands::delete_profile,
            commands::rename_profile,
            commands::duplicate_profile,
            commands::save_device_profile,
            commands::get_device_profile,
            commands::save_settings,
            commands::get_config_path,
            commands::window_minimize,
            commands::window_toggle_maximize,
            commands::window_close,
        ])
        .build(tauri::generate_context!())
        .expect("error while building OpenGHub")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<std::sync::Arc<scripting::Scripting>>().stop();
                app.state::<std::sync::Arc<lightsync::LightSync>>().stop_all();
                app.state::<DeviceManager>().release_devices();
                app.state::<std::sync::Arc<remap::Injector>>().release_all();
            }
        });
}

/// Emitted with a route when a tray menu entry is chosen; the layout navigates.
pub const EVENT_NAVIGATE: &str = "navigate";

/// The tray icon with G HUB's menu: a block per connected device (name, then
/// connection and battery), the four sections, and Close.
fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let menu = tray_menu(app.handle())?;
    // A light variant of the logo: the app icon's black disc would vanish
    // on the dark panels most desktops use.
    let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray/app.png"))
        .ok()
        .or_else(|| app.default_window_icon().cloned())
        .ok_or_else(|| tauri::Error::AssetNotFound("tray icon".into()))?;
    TrayIconBuilder::with_id("main")
        .icon(tray_icon)
        .tooltip("OpenGHub")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            if let Some(route) = id.strip_prefix("nav:") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
                let _ = app.emit(EVENT_NAVIGATE, route.to_string());
            } else if id == "quit" {
                // Give the devices back before going: identity remapping,
                // spy off, onboard mode, released keys.
                app.state::<std::sync::Arc<scripting::Scripting>>().stop();
                app.state::<DeviceManager>().release_devices();
                app.state::<std::sync::Arc<remap::Injector>>().release_all();
                app.exit(0)
            }
        })
        .build(app)?;
    Ok(())
}

/// Rebuilds the tray menu from the current device snapshots — after a rescan
/// and after every battery reading, so the header stays truthful.
pub fn refresh_tray_menu(app: &tauri::AppHandle) {
    if let Some(tray) = app.tray_by_id("main") {
        match tray_menu(app) {
            Ok(menu) => {
                if let Err(e) = tray.set_menu(Some(menu)) {
                    log::debug!("tray menu not updated: {e}");
                }
            }
            Err(e) => log::debug!("tray menu not built: {e}"),
        }
    }
}

fn tray_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    use tauri::image::Image;
    use tauri::menu::{IconMenuItem, PredefinedMenuItem};

    let icon = |png: &'static [u8]| Image::from_bytes(png).ok();
    let menu = Menu::new(app)?;

    // One block per device, as G HUB lists them: the name in its own row,
    // then the connection and the battery. Both rows are inert.
    let manager = app.state::<DeviceManager>();
    let mut any = false;
    for snap in manager.snapshots() {
        if snap.demo || !snap.online || matches!(snap.kind, hidpp::registry::DeviceKind::Receiver) {
            continue;
        }
        any = true;
        menu.append(&MenuItem::with_id(app, format!("dev:{}", snap.id), &snap.name, false, None::<&str>)?)?;
        let (conn_icon, conn_label) = match snap.connection {
            state::Connection::Wired => (icon(include_bytes!("../icons/tray/usb.png")), "WIRED"),
            state::Connection::Bluetooth => (icon(include_bytes!("../icons/tray/bluetooth.png")), "BLUETOOTH"),
            state::Connection::Wireless | state::Connection::Receiver => (icon(include_bytes!("../icons/tray/wireless.png")), "LIGHTSPEED"),
        };
        let mut line = conn_label.to_string();
        if let Some(b) = &snap.battery {
            let charging = matches!(
                b.status,
                hidpp::features::ChargeStatus::Charging | hidpp::features::ChargeStatus::SlowCharging | hidpp::features::ChargeStatus::ChargingFull
            );
            line = format!("{line}     {}{}%", if charging { "⚡ " } else { "" }, b.percentage);
        }
        menu.append(&IconMenuItem::with_id(app, format!("status:{}", snap.id), &line, false, conn_icon, None::<&str>)?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    if !any {
        menu.append(&MenuItem::with_id(app, "dev:none", "No devices connected", false, None::<&str>)?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }

    for (route, label, png) in [
        ("/", "Devices", &include_bytes!("../icons/tray/devices.png")[..]),
        ("/games", "Games", &include_bytes!("../icons/tray/games.png")[..]),
        ("/community", "Community", &include_bytes!("../icons/tray/community.png")[..]),
        ("/profiles", "Profiles", &include_bytes!("../icons/tray/profiles.png")[..]),
    ] {
        menu.append(&IconMenuItem::with_id(app, format!("nav:{route}"), label, true, icon(png), None::<&str>)?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", "Close OpenGHub", true, None::<&str>)?)?;
    Ok(menu)
}

/// Emitted after artwork was fetched for a device, so the UI rescans.
pub const EVENT_ARTWORK_CHANGED: &str = "artwork-changed";

/// Fetches renders for any connected device that has none — what G HUB does the
/// first time it sees a device. Silent when no depository has been imported,
/// and never retries a product id that already failed this session.
pub fn spawn_artwork_fetch(app: tauri::AppHandle, devices: Vec<state::DeviceSnapshot>) {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    static TRIED: OnceLock<Mutex<HashSet<u16>>> = OnceLock::new();

    tauri::async_runtime::spawn_blocking(move || {
        if !app.state::<Store>().get().settings.auto_fetch_artwork {
            return;
        }
        if depot::load_cache().is_err() {
            return; // nothing imported yet; the settings screen explains
        }
        let have = artwork::scan();
        let tried = TRIED.get_or_init(|| Mutex::new(HashSet::new()));
        let mut fetched_any = false;

        for device in devices.iter().filter(|d| d.online && !d.demo) {
            let mut ids = device.model_ids.clone();
            ids.push(device.product_id);
            if ids.iter().any(|id| have.contains_key(&artwork::key(*id))) {
                continue;
            }
            {
                let mut t = tried.lock().unwrap_or_else(|e| e.into_inner());
                if ids.iter().any(|id| t.contains(id)) {
                    continue;
                }
                t.extend(ids.iter().copied());
            }
            match depot::fetch_for_product_ids(&ids) {
                Ok(d) => {
                    log::info!("fetched artwork for {}", d.display_name);
                    fetched_any = true;
                }
                Err(e) => log::warn!("artwork for {} not fetched: {e}", device.name),
            }
        }
        if fetched_any {
            let _ = app.emit(EVENT_ARTWORK_CHANGED, ());
        }
    });
}

/// Loads Logitech's application database, then watches for a bound game
/// starting or stopping and switches the active profile to match — G HUB's
/// per-game profiles. Detection reads `/proc`, so it costs a few milliseconds
/// every couple of seconds and works under any compositor.
fn spawn_application_watcher(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let db = app.state::<std::sync::Arc<apps::AppDatabase>>().inner().clone();
        let loaded = tauri::async_runtime::spawn_blocking({
            let db = db.clone();
            move || db.load()
        })
        .await;
        match loaded {
            Ok(Ok(info)) => log::info!(
                "application database {} ({} apps)",
                info.version,
                info.application_count
            ),
            Ok(Err(e)) => log::warn!("application database unavailable: {e}"),
            Err(e) => log::warn!("application database task failed: {e}"),
        }

        let mut last: Option<String> = None;
        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            let current = db.detect_running();
            if current == last {
                continue;
            }
            last = current.clone();
            let _ = app.emit(commands::EVENT_ACTIVE_APPLICATION, &current);

            // Switch profiles to match, if the user has that on.
            let store = app.state::<Store>();
            let cfg = store.get();
            if !cfg.settings.auto_switch_profiles {
                continue;
            }
            let persistent = cfg.settings.persistent_profile.as_str();
            let target = match &current {
                Some(id) => cfg
                    .settings
                    .active_profile_per_app
                    .get(id)
                    .and_then(|pid| cfg.profiles.iter().find(|p| &p.id == pid && !p.disabled))
                    .or_else(|| cfg.profiles.iter().find(|p| !p.disabled && p.application_id.as_deref() == Some(id)))
                    // A disabled game behaves as if it were not running.
                    .or_else(|| cfg.profiles.iter().find(|p| p.id == persistent)),
                None => cfg.profiles.iter().find(|p| p.id == persistent),
            };
            if let Some(profile) = target {
                if profile.id != cfg.active_profile {
                    log::info!("switching to profile '{}'", profile.name);
                    let id = profile.id.clone();
                    let _ = store.update(|c| c.active_profile = id);
                    let _ = app.emit(commands::EVENT_CONFIG_CHANGED, store.get());
                    apply_all_profiles(&app);
                }
            }
        }
    });
}

/// Pushes the active profile's wheel settings to every connected wheel and
/// starts the force-feedback driver when it is enabled. Called after every
/// rescan and profile switch, so a wheel always runs what the profile says.
pub fn apply_wheel_profiles(app: &tauri::AppHandle) {
    let manager = app.state::<DeviceManager>();
    let store = app.state::<Store>();
    let cfg = store.get();
    for id in manager.wheel_ids() {
        let settings = cfg
            .profiles
            .iter()
            .find(|p| p.id == cfg.active_profile)
            .and_then(|p| p.devices.get(&id))
            .and_then(|d| d.wheel.clone())
            .unwrap_or_default();
        if let Err(e) = manager.apply_wheel_settings(&id, &settings) {
            log::warn!("wheel {id}: settings not applied: {e}");
        }
        let was_running = manager.snapshot(&id).and_then(|s| s.wheel).map(|w| w.driver_running).unwrap_or(false);
        match manager.set_wheel_driver(&id, cfg.settings.wheel_driver) {
            Ok(true) if !was_running => log::info!("wheel {id}: force-feedback driver running"),
            Ok(_) => {}
            Err(e) => log::warn!("wheel {id}: force-feedback driver not started: {e}"),
        }
    }
}

/// Writes each device's power timeouts into its onboard profile. `only`
/// limits it to one device.
pub fn apply_device_settings(app: &tauri::AppHandle, only: Option<&str>) {
    let manager = app.state::<DeviceManager>();
    let store = app.state::<Store>();
    let cfg = store.get();
    for snap in manager.snapshots() {
        if snap.demo || !snap.online || !snap.capabilities.onboard_memory {
            continue;
        }
        if only.is_some_and(|o| o != snap.id) {
            continue;
        }
        let Some(ds) = cfg.settings.device_settings.get(&snap.id) else { continue };
        if let Err(e) = manager.write_onboard_power(&snap.id, ds.inactivity_lighting_min, ds.auto_sleep_min) {
            log::warn!("{}: power settings not written: {e}", snap.name);
        }
    }
}

/// Pushes the active profile's button assignments to every connected device.
pub fn apply_assignment_profiles(app: &tauri::AppHandle) {
    let manager = app.state::<DeviceManager>();
    let store = app.state::<Store>();
    let cfg = store.get();
    let active = cfg.profiles.iter().find(|p| p.id == cfg.active_profile);
    let script_active = active.and_then(|p| p.script.as_deref()).map(|s| !s.trim().is_empty()).unwrap_or(false);
    for snap in manager.snapshots() {
        if snap.demo || !snap.online {
            continue;
        }
        let dp = active.and_then(|p| p.devices.get(&snap.id));
        let mut assignments = dp.map(|d| d.assignments.clone()).unwrap_or_default();
        // Left-handed layout: swap the two clicks unless the profile says otherwise.
        if cfg.settings.device_settings.get(&snap.id).map(|d| d.left_handed).unwrap_or(false) {
            for (control, value, label) in [("button-1", "mouse-right", "Secondary Click"), ("button-2", "mouse-left", "Primary Click")] {
                if !assignments.iter().any(|a| a.control == control) {
                    assignments.push(profiles::Assignment {
                        control: control.into(),
                        category: "action".into(),
                        label: label.into(),
                        value: value.into(),
                    });
                }
            }
        }
        let macros = dp.map(|d| d.macros.clone()).unwrap_or_default();
        match manager.apply_assignments(&snap.id, &assignments, &macros, script_active) {
            Ok(r) if r.software || r.onboard => log::info!(
                "{}: {} assignment(s) applied (software: {}, onboard: {})",
                snap.name,
                assignments.len(),
                r.software,
                r.onboard
            ),
            Ok(_) => {}
            Err(e) => log::debug!("{}: assignments not applied: {e}", snap.name),
        }
    }
}

/// Polls every device with software-mode assignments for button edges and
/// performs the assigned actions. 4 ms keeps click-to-key latency invisible.
fn spawn_button_pump(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("button-pump".into())
        .spawn(move || loop {
            std::thread::sleep(Duration::from_millis(4));
            // A panic here would silently kill every assignment; keep pumping.
            let tick = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let events = app.state::<DeviceManager>().pump_button_events();
                if events.is_empty() {
                    return;
                }
                let scripting = app.state::<std::sync::Arc<scripting::Scripting>>();
                for ev in events {
                    if !ev.wheel {
                        scripting.mouse_button(ev.button, ev.pressed);
                    }
                    if ev.action.is_some() {
                        perform(&app, ev);
                    }
                }
            }));
            if tick.is_err() {
                log::error!("button pump tick panicked; continuing");
                std::thread::sleep(Duration::from_millis(250));
            }
        })
        .expect("button pump thread");
}

/// Runs one assignment edge. Keys are held for as long as the button is.
fn perform(app: &tauri::AppHandle, ev: state::ButtonEvent) {
    use remap::Action;
    let injector = app.state::<std::sync::Arc<remap::Injector>>().inner().clone();
    let Some(action) = ev.action.as_ref() else { return };
    let result: std::io::Result<()> = match action {
        Action::Keys(codes) => injector.chord(codes, ev.pressed),
        Action::MouseButton(n) => injector.mouse_button(*n, ev.pressed),
        Action::Macro(steps) => {
            if ev.pressed {
                injector.play_macro(steps.clone());
            }
            Ok(())
        }
        Action::LockScreen => {
            if ev.pressed {
                remap::lock_screen();
            }
            Ok(())
        }
        Action::DpiUp | Action::DpiDown | Action::DpiCycle | Action::DpiDefault | Action::DpiShift => {
            dpi_action(app, &ev.device_id, action, ev.pressed);
            Ok(())
        }
        Action::ProfileNext => {
            if ev.pressed {
                next_profile(app);
            }
            Ok(())
        }
        Action::GShift | Action::Disabled => Ok(()),
    };
    if let Err(e) = result {
        log::warn!("assignment on {} button {} failed: {e}", ev.device_id, ev.button + 1);
    }
}

/// DPI up / down / cycle / default / shift against the profile's stage list,
/// written to the device and saved back so the Sensitivity page follows.
fn dpi_action(app: &tauri::AppHandle, device_id: &str, action: &remap::Action, pressed: bool) {
    use remap::Action;
    let store = app.state::<Store>();
    let manager = app.state::<DeviceManager>();
    let cfg = store.get();
    let Some(profile) = cfg.profiles.iter().find(|p| p.id == cfg.active_profile) else { return };
    let dp = profile.devices.get(device_id).cloned().unwrap_or_default();
    if dp.dpi_stages.is_empty() {
        return;
    }
    let last = dp.dpi_stages.len() - 1;
    let current = dp.active_stage.min(last);

    // Shift is momentary: jump on press, come back on release, stage unchanged.
    if let Action::DpiShift = action {
        let shift = dp.shift_stage.unwrap_or(0).min(last);
        let target = if pressed { dp.dpi_stages[shift] } else { dp.dpi_stages[current] };
        if let Err(e) = manager.set_dpi(device_id, target) {
            log::warn!("dpi shift failed: {e}");
        }
        if let Some(snap) = manager.snapshot(device_id) {
            let _ = app.emit(commands::EVENT_DEVICE_UPDATED, snap);
        }
        return;
    }
    if !pressed {
        return;
    }
    let next = match action {
        Action::DpiUp => (current + 1).min(last),
        Action::DpiDown => current.saturating_sub(1),
        Action::DpiCycle => (current + 1) % (last + 1),
        Action::DpiDefault => {
            let default = manager.snapshot(device_id).and_then(|s| s.dpi).map(|d| d.default).unwrap_or(dp.dpi_stages[0]);
            dp.dpi_stages.iter().enumerate().min_by_key(|(_, v)| v.abs_diff(default)).map(|(i, _)| i).unwrap_or(0)
        }
        _ => current,
    };
    if next == current && !matches!(action, Action::DpiDefault) {
        return;
    }
    match manager.set_dpi(device_id, dp.dpi_stages[next]) {
        Ok(_) => {
            let id = device_id.to_string();
            let pid = profile.id.clone();
            let _ = store.update(|c| {
                if let Some(p) = c.profiles.iter_mut().find(|p| p.id == pid) {
                    if let Some(d) = p.devices.get_mut(&id) {
                        d.active_stage = next;
                    }
                }
            });
            let _ = app.emit(commands::EVENT_CONFIG_CHANGED, store.get());
            if let Some(snap) = manager.snapshot(device_id) {
                let _ = app.emit(commands::EVENT_DEVICE_UPDATED, snap);
            }
        }
        Err(e) => log::warn!("dpi action failed: {e}"),
    }
}

/// Cycles to the next enabled profile, as G HUB's "profile cycle" does.
fn next_profile(app: &tauri::AppHandle) {
    let store = app.state::<Store>();
    let cfg = store.get();
    let enabled: Vec<&profiles::Profile> = cfg.profiles.iter().filter(|p| !p.disabled).collect();
    if enabled.len() < 2 {
        return;
    }
    let pos = enabled.iter().position(|p| p.id == cfg.active_profile).unwrap_or(0);
    let next = enabled[(pos + 1) % enabled.len()].id.clone();
    let _ = store.update(|c| c.active_profile = next);
    let _ = app.emit(commands::EVENT_CONFIG_CHANGED, store.get());
    apply_all_profiles(app);
}

/// Everything a profile switch or rescan has to push to the hardware.
pub fn apply_all_profiles(app: &tauri::AppHandle) {
    apply_wheel_profiles(app);
    apply_onboard_modes(app);
    apply_device_settings(app, None);
    apply_assignment_profiles(app);
    apply_lighting_profiles(app);
    for id in app.state::<Store>().get().settings.onboard_mode_devices.clone() {
        write_profile_to_device(app, &id);
    }
    apply_profile_script(app);
}

/// The channel scripts use to reach the device layer.
pub struct ScriptHost(pub std::sync::mpsc::Sender<scripting::HostRequest>);

/// Runs the active profile's Lua script; stops the previous one when the
/// profile changed or has no script. Restarting the same profile's script is
/// left to `set_profile_script`, so a rescan does not re-run it.
pub fn apply_profile_script(app: &tauri::AppHandle) {
    let cfg = app.state::<Store>().get();
    let scripting = app.state::<std::sync::Arc<scripting::Scripting>>();
    let active = cfg.profiles.iter().find(|p| p.id == cfg.active_profile);
    let source = active.and_then(|p| p.script.as_deref()).filter(|s| !s.trim().is_empty());
    match (source, scripting.active_profile()) {
        (Some(_), Some(running)) if running == cfg.active_profile => {}
        (Some(src), _) => start_script(app, &cfg.active_profile, src),
        (None, Some(_)) => scripting.stop(),
        (None, None) => {}
    }
}

pub fn start_script(app: &tauri::AppHandle, profile_id: &str, source: &str) {
    let scripting = app.state::<std::sync::Arc<scripting::Scripting>>();
    let injector = app.state::<std::sync::Arc<remap::Injector>>().inner().clone();
    let host = app.state::<ScriptHost>().0.clone();
    scripting.start(profile_id, source, injector, host);
}

/// Serves what a script asks of the devices: DPI, backlight, macros. One
/// thread, so a chatty script cannot stall the pump or the UI.
fn spawn_script_host(app: tauri::AppHandle) -> std::sync::mpsc::Sender<scripting::HostRequest> {
    use scripting::HostRequest;
    let (tx, rx) = std::sync::mpsc::channel::<HostRequest>();
    std::thread::Builder::new()
        .name("script-host".into())
        .spawn(move || {
            for req in rx {
                match req {
                    HostRequest::SetDpiIndex(i) => script_set_dpi(&app, None, i),
                    HostRequest::SetDpiTable(table, i) => script_set_dpi(&app, Some(table), i),
                    HostRequest::SetBacklight(rgb) => {
                        let manager = app.state::<DeviceManager>();
                        for snap in manager.snapshots() {
                            if snap.demo || !snap.online || !snap.capabilities.lighting {
                                continue;
                            }
                            let zones = manager.lighting_zones(&snap.id).unwrap_or_default();
                            for z in zones {
                                let _ = manager.set_lighting(&snap.id, z.index, rgb, hidpp::features::LightEffect::Fixed, false);
                            }
                        }
                    }
                    HostRequest::PlayMacro(name) => {
                        let cfg = app.state::<Store>().get();
                        let steps = cfg
                            .profiles
                            .iter()
                            .find(|p| p.id == cfg.active_profile)
                            .and_then(|p| p.devices.values().flat_map(|d| d.macros.iter()).find(|m| m.name.eq_ignore_ascii_case(&name)))
                            .map(|m| m.steps.clone());
                        match steps {
                            Some(steps) => app.state::<std::sync::Arc<remap::Injector>>().inner().play_macro(steps),
                            None => log::warn!("script: no macro named {name:?} in the active profile"),
                        }
                    }
                }
            }
        })
        .expect("script host thread");
    tx
}

/// `SetMouseDPITableIndex` / `SetMouseDPITable` for every mouse in the
/// active profile; the Sensitivity page follows through the config event.
fn script_set_dpi(app: &tauri::AppHandle, table: Option<Vec<u16>>, index: usize) {
    let store = app.state::<Store>();
    let manager = app.state::<DeviceManager>();
    let cfg = store.get();
    let pid = cfg.active_profile.clone();
    let mut changed = false;
    for snap in manager.snapshots() {
        if snap.demo || !snap.online || !snap.capabilities.dpi {
            continue;
        }
        let mut dp = cfg.profiles.iter().find(|p| p.id == pid).and_then(|p| p.devices.get(&snap.id)).cloned().unwrap_or_default();
        if let Some(t) = &table {
            dp.dpi_stages = t.clone();
        }
        if dp.dpi_stages.is_empty() {
            continue;
        }
        let stage = index.min(dp.dpi_stages.len() - 1);
        match manager.set_dpi(&snap.id, dp.dpi_stages[stage]) {
            Ok(_) => {
                let id = snap.id.clone();
                let stages = dp.dpi_stages.clone();
                let _ = store.update(|c| {
                    if let Some(p) = c.profiles.iter_mut().find(|p| p.id == pid) {
                        let d = p.devices.entry(id).or_default();
                        d.dpi_stages = stages;
                        d.active_stage = stage;
                    }
                });
                changed = true;
                if let Some(s) = manager.snapshot(&snap.id) {
                    let _ = app.emit(commands::EVENT_DEVICE_UPDATED, s);
                }
            }
            Err(e) => log::warn!("script: dpi not set on {}: {e}", snap.name),
        }
    }
    if changed {
        let _ = app.emit(commands::EVENT_CONFIG_CHANGED, store.get());
    }
}

/// Puts the devices the user chose into on-board memory mode after a rescan,
/// and writes the active profile into them. Everything else stays host-driven.
pub fn apply_onboard_modes(app: &tauri::AppHandle) {
    let manager = app.state::<DeviceManager>();
    let store = app.state::<Store>();
    let chosen = store.get().settings.onboard_mode_devices;
    for snap in manager.snapshots() {
        if snap.demo || !snap.capabilities.onboard_memory {
            continue;
        }
        let want = chosen.contains(&snap.id);
        if want && snap.onboard_mode != Some(true) {
            match manager.set_onboard_mode(&snap.id, true) {
                Ok(_) => write_profile_to_device(app, &snap.id),
                Err(e) => log::warn!("{}: onboard mode not set: {e}", snap.name),
            }
        } else if !want && snap.onboard_mode == Some(true) {
            // Not chosen here (or chosen in G HUB on another OS): host mode
            // while we run, so assignments and scripts see the buttons. Quit
            // hands the device back to its onboard profile.
            if let Err(e) = manager.set_onboard_mode(&snap.id, false) {
                log::warn!("{}: host mode not set: {e}", snap.name);
            }
        }
    }
}

/// Writes the active profile's DPI ladder and lighting into a device's onboard
/// profile — "effects on device". Assignments go through `apply_assignments`.
pub fn write_profile_to_device(app: &tauri::AppHandle, device_id: &str) {
    let manager = app.state::<DeviceManager>();
    let store = app.state::<Store>();
    let cfg = store.get();
    let Some(dp) = cfg.profiles.iter().find(|p| p.id == cfg.active_profile).and_then(|p| p.devices.get(device_id)) else {
        return;
    };
    if !dp.dpi_stages.is_empty() {
        if let Err(e) = manager.write_onboard_dpi(device_id, &dp.dpi_stages, dp.active_stage, dp.shift_stage, dp.report_rate_hz) {
            log::warn!("{device_id}: onboard DPI not written: {e}");
        }
    }
    let mut zones: Vec<(u8, [u8; 3], hidpp::features::LightEffect)> = Vec::new();
    for (zone, l) in &dp.lighting_zones {
        let Ok(z) = zone.parse::<u8>() else { continue };
        let rgb = crate::lightsync::parse_hex(&l.color);
        let fx = match l.effect.as_str() {
            "off" => hidpp::features::LightEffect::Off,
            "breathing" => hidpp::features::LightEffect::Breathing { rate_ms: l.rate_ms, brightness: l.brightness },
            "cycle" => hidpp::features::LightEffect::Cycle { rate_ms: l.rate_ms, brightness: l.brightness },
            // Software effects cannot live on the device; keep a fixed colour.
            _ => hidpp::features::LightEffect::Fixed,
        };
        zones.push((z, rgb, fx));
    }
    if !zones.is_empty() {
        if let Err(e) = manager.write_onboard_lighting(device_id, &zones) {
            log::warn!("{device_id}: onboard lighting not written: {e}");
        }
    }
}

/// Installs the active profile's software lighting effects (screen sampler,
/// audio visualizer) for the connected devices; firmware effects are already
/// on the device. The restore token for screen sharing is loaded from settings.
pub fn apply_lighting_profiles(app: &tauri::AppHandle) {
    let manager = app.state::<DeviceManager>();
    let store = app.state::<Store>();
    let sync = app.state::<std::sync::Arc<lightsync::LightSync>>();
    let cfg = store.get();
    if sync.restore_token.lock().is_none() {
        *sync.restore_token.lock() = cfg.settings.screen_restore_token.clone();
    }
    let active = cfg.profiles.iter().find(|p| p.id == cfg.active_profile);
    let mut table = std::collections::HashMap::new();
    for snap in manager.snapshots() {
        // Wheels have no colour zones but their RPM LEDs take software effects.
        if snap.demo || !(snap.capabilities.lighting || snap.capabilities.wheel) {
            continue;
        }
        let Some(dp) = active.and_then(|p| p.devices.get(&snap.id)) else { continue };
        for (zone, settings) in &dp.lighting_zones {
            if let (Ok(z), Some(fx)) = (zone.parse::<u8>(), settings.software.clone()) {
                if settings.effect == "screen" || settings.effect == "audio" {
                    table.insert((snap.id.clone(), z), fx);
                }
            }
        }
    }
    sync.set_effects(table);
}

/// G HUB's low-battery mode: below the threshold the lighting is dimmed to
/// the configured brightness; once charged past it (with a little
/// hysteresis) the profile's lighting comes back.
fn low_battery_check(app: &tauri::AppHandle, device_id: &str, percentage: u8, dimmed: &mut std::collections::HashSet<String>) {
    let store = app.state::<Store>();
    let cfg = store.get();
    let Some(ds) = cfg.settings.device_settings.get(device_id) else { return };
    if !ds.low_battery_mode {
        if dimmed.remove(device_id) {
            write_profile_lighting(app, device_id, None);
        }
        return;
    }
    let is_dimmed = dimmed.contains(device_id);
    if percentage <= ds.low_battery_threshold && !is_dimmed {
        log::info!("{device_id}: battery {percentage}% — low-battery mode dims the lighting");
        write_profile_lighting(app, device_id, Some(ds.low_battery_brightness));
        dimmed.insert(device_id.to_string());
    } else if percentage > ds.low_battery_threshold.saturating_add(5) && is_dimmed {
        write_profile_lighting(app, device_id, None);
        dimmed.remove(device_id);
    }
}

/// Re-applies the active profile's lighting to a device, optionally with the
/// brightness capped (low-battery mode). RAM only.
fn write_profile_lighting(app: &tauri::AppHandle, device_id: &str, cap: Option<u8>) {
    let manager = app.state::<DeviceManager>();
    let store = app.state::<Store>();
    let cfg = store.get();
    let Some(dp) = cfg.profiles.iter().find(|p| p.id == cfg.active_profile).and_then(|p| p.devices.get(device_id)) else {
        return;
    };
    for (zone, l) in &dp.lighting_zones {
        let Ok(z) = zone.parse::<u8>() else { continue };
        let brightness = cap.map(|c| c.min(l.brightness)).unwrap_or(l.brightness);
        let mut rgb = crate::lightsync::parse_hex(&l.color);
        let fx = match l.effect.as_str() {
            "off" => hidpp::features::LightEffect::Off,
            "breathing" => hidpp::features::LightEffect::Breathing { rate_ms: l.rate_ms, brightness },
            "cycle" => hidpp::features::LightEffect::Cycle { rate_ms: l.rate_ms, brightness },
            _ => {
                // Fixed (and software effects): scale the colour itself.
                for c in rgb.iter_mut() {
                    *c = (*c as u32 * brightness as u32 / 100) as u8;
                }
                hidpp::features::LightEffect::Fixed
            }
        };
        if let Err(e) = manager.set_lighting(device_id, z, rgb, fx, false) {
            log::debug!("{device_id} zone {z}: lighting not written: {e}");
        }
    }
}

/// Streams wheel inputs (steering, pedals, buttons) to the frontend at ~30 Hz
/// while they change, for the Steering Wheel page's live gauge.
fn spawn_wheel_telemetry(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("wheel-telemetry".into())
        .spawn(move || {
            let mut last: std::collections::HashMap<String, wheel::WheelState> = Default::default();
            loop {
                std::thread::sleep(Duration::from_millis(33));
                let manager = app.state::<DeviceManager>();
                for id in manager.wheel_ids() {
                    let Ok(state) = manager.wheel_state(&id) else { continue };
                    if last.get(&id) == Some(&state) {
                        continue;
                    }
                    last.insert(id.clone(), state);
                    let _ = app.emit(
                        commands::EVENT_WHEEL_STATE,
                        commands::WheelStateEvent { device_id: id, state },
                    );
                }
            }
        })
        .expect("telemetry thread");
}

/// Background worker that polls battery-backed devices and pushes the result to
/// the frontend. Runs on a blocking thread because hidapi I/O is synchronous.
fn spawn_battery_poller(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Devices currently dimmed by low-battery mode.
        let mut dimmed: std::collections::HashSet<String> = Default::default();
        loop {
            let interval = {
                let store = app.state::<Store>();
                let secs = store.get().settings.battery_poll_seconds;
                Duration::from_secs(secs.max(MIN_POLL_SECONDS))
            };
            tokio::time::sleep(interval).await;

            let app_for_poll = app.clone();
            let events = tauri::async_runtime::spawn_blocking(move || {
                let manager = app_for_poll.state::<DeviceManager>();
                manager.poll_batteries()
            })
            .await;

            match events {
                Ok(events) => {
                    let any = !events.is_empty();
                    for (device_id, battery) in events {
                        low_battery_check(&app, &device_id, battery.percentage, &mut dimmed);
                        let _ = app.emit(EVENT_BATTERY, BatteryEvent { device_id, battery });
                    }
                    if any {
                        refresh_tray_menu(&app);
                    }
                }
                Err(e) => log::warn!("battery poll task failed: {e}"),
            }
        }
    });
}
