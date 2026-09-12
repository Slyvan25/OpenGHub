//! OpenGHub — an open source, Linux-native reimplementation of Logitech G HUB.

pub mod artwork;
pub mod commands;
pub mod demo;
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
        .setup(|app| {
            let handle = app.handle().clone();

            // Initial scan so the dashboard has data before the first IPC call.
            let manager = app.state::<DeviceManager>();
            let devices = manager.refresh();
            log::info!("found {} device(s) (demo: {})", devices.len(), manager.is_demo());

            build_tray(app)?;
            spawn_battery_poller(handle);
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
            commands::backup_onboard_memory,
            commands::get_onboard_profiles,
            commands::apply_onboard_macros,
            commands::restore_onboard_memory,
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
