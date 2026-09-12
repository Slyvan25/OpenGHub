//! OpenGHub — an open source, Linux-native reimplementation of Logitech G HUB.

pub mod apps;
pub mod artwork;
pub mod commands;
pub mod demo;
pub mod depot;
pub mod hidpp;
pub mod profiles;
pub mod state;

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
        .setup(|app| {
            let handle = app.handle().clone();

            // Initial scan so the dashboard has data before the first IPC call.
            let manager = app.state::<DeviceManager>();
            let devices = manager.refresh();
            log::info!("found {} device(s) (demo: {})", devices.len(), manager.is_demo());
            spawn_artwork_fetch(handle.clone(), devices);

            build_tray(app)?;
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
            commands::save_device_profile,
            commands::get_device_profile,
            commands::save_settings,
            commands::get_config_path,
            commands::window_minimize,
            commands::window_toggle_maximize,
            commands::window_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running OpenGHub");
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Open OpenGHub", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().ok_or_else(|| {
            tauri::Error::AssetNotFound("default window icon".into())
        })?)
        .tooltip("OpenGHub")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
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
            let target = match &current {
                Some(id) => cfg.profiles.iter().find(|p| p.application_id.as_deref() == Some(id)),
                None => cfg.profiles.iter().find(|p| p.id == "default"),
            };
            if let Some(profile) = target {
                if profile.id != cfg.active_profile {
                    log::info!("switching to profile '{}'", profile.name);
                    let id = profile.id.clone();
                    let _ = store.update(|c| c.active_profile = id);
                    let _ = app.emit("config-changed", store.get());
                }
            }
        }
    });
}

/// Background worker that polls battery-backed devices and pushes the result to
/// the frontend. Runs on a blocking thread because hidapi I/O is synchronous.
fn spawn_battery_poller(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
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
                    for (device_id, battery) in events {
                        let _ = app.emit(EVENT_BATTERY, BatteryEvent { device_id, battery });
                    }
                }
                Err(e) => log::warn!("battery poll task failed: {e}"),
            }
        }
    });
}
