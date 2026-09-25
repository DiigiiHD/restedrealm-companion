//! Keeping the app itself current. A release carries a signed update file on
//! GitHub; the app checks it after start and every six hours, installs a newer
//! version quietly (a per-user install needs no permission prompt) and starts
//! again. An update whose signature does not match the key built into the app
//! is refused by the updater plugin.

use crate::state::Shared;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

const FIRST_CHECK: Duration = Duration::from_secs(60);
const EVERY: Duration = Duration::from_secs(6 * 3600);

/// Release builds get the update key from the release workflow. A local build
/// has none, and then never looks for updates.
pub fn enabled(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .is_some_and(|k| !k.trim().is_empty())
}

fn set(app: &AppHandle, shared: &Shared, text: Option<String>) {
    shared.live.lock().unwrap().update = text;
    let _ = app.emit("state-changed", ());
}

/// Look for a newer version and install it. Returns a line for the Settings screen.
pub async fn check_and_install(app: &AppHandle, shared: &Shared) -> Result<String, String> {
    if !enabled(app) {
        return Ok("This build does not update itself.".into());
    }
    let updater = app.updater().map_err(|e| e.to_string())?;
    let Some(update) =
        updater.check().await.map_err(|_| "Could not check for updates. It tries again later.".to_string())?
    else {
        set(app, shared, None);
        return Ok("You have the newest version.".into());
    };
    set(app, shared, Some(format!("Installing version {}", update.version)));
    // On Windows the installer closes the app and starts the new version.
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|_| "The update could not be installed. It tries again later.".to_string())?;
    app.restart();
}

pub fn start(app: AppHandle, shared: Arc<Shared>) {
    if !enabled(&app) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio_sleep(FIRST_CHECK).await;
        loop {
            if let Err(e) = check_and_install(&app, &shared).await {
                set(&app, &shared, None);
                eprintln!("{e}");
            }
            tokio_sleep(EVERY).await;
        }
    });
}

async fn tokio_sleep(duration: Duration) {
    // Tauri's async runtime is Tokio; a blocking sleep on a helper thread keeps
    // this file free of a direct Tokio dependency.
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(duration)).await;
}
