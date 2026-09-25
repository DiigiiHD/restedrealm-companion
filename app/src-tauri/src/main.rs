// No console window behind the app in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod addon;
mod commands;
mod labels;
mod state;
mod tray;
mod worker;

use state::{Live, Shared};
use std::sync::{mpsc, Arc, Mutex};
use tauri::path::BaseDirectory;
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

/// Passed by the start-with-Windows entry so the app starts next to the clock.
const BACKGROUND_ARG: &str = "--background";

fn main() {
    tauri::Builder::default()
        // A second launch shows the running app instead of starting another.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_window(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![BACKGROUND_ARG])))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let local = app.path().local_data_dir()?;
            let state_dir = local.join("RestedRealm Companion");
            // Carry the Python pilot's queue and backups over once; the old folder stays.
            if let Err(e) =
                rrc_core::queue::migrate_legacy(&local.join("RestedRealmCollector").join("Companion"), &state_dir)
            {
                eprintln!("Pilot queue was not copied: {e}");
            }
            let addon_source = app.path().resolve("addon/RestedRealmCollector", BaseDirectory::Resource)?;
            let (tx, rx) = mpsc::channel();
            let shared = Arc::new(Shared {
                state_dir,
                addon_source,
                store: rrc_core::credentials::platform_store(),
                live: Mutex::new(Live::default()),
                trigger: Mutex::new(tx.clone()),
                tray_status: Mutex::new(None),
            });
            app.manage(shared.clone());
            tray::build(app.handle(), shared.clone())?;

            let handle = app.handle().clone();
            let worker_shared = shared.clone();
            std::thread::Builder::new()
                .name("companion-worker".into())
                .spawn(move || worker::run(handle, worker_shared, rx, tx))?;

            let background = std::env::args().any(|a| a == BACKGROUND_ARG);
            if !background || !shared.flag(state::keys::SETUP_DONE) {
                tray::show_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the companion running next to the clock.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::detect_game,
            commands::set_game_dir,
            commands::install_addon,
            commands::pair,
            commands::set_auto_upload,
            commands::set_autostart,
            commands::finish_setup,
            commands::sync_now,
            commands::recent_records,
            commands::disconnect,
            commands::forget_local_data,
            commands::open_page,
        ])
        .run(tauri::generate_context!())
        .expect("RestedRealm Companion could not start");
}
