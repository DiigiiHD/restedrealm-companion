//! Everything the window may ask for. Each command does one fixed thing; the
//! window cannot name arbitrary URLs, files or queries.

use crate::addon;
use crate::labels::{kind_group, kind_label, GROUP_ORDER};
use crate::state::{keys, Shared, Trigger};
use crate::worker::now;
use rrc_core::upload::{redact, redeem_code, Https};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

type SharedState<'a> = State<'a, Arc<Shared>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    label: &'static str,
    count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateView {
    version: &'static str,
    setup_done: bool,
    connected: bool,
    auto_upload: bool,
    autostart: bool,
    game_dir: Option<String>,
    saves_found: usize,
    addon_version: Option<String>,
    bundled_addon_version: Option<String>,
    wow_running: bool,
    working: bool,
    last_error: Option<String>,
    observations: i64,
    pending: i64,
    last_upload_at: Option<i64>,
    last_upload_count: Option<i64>,
    this_week: Vec<Group>,
}

#[tauri::command]
pub async fn get_state(app: AppHandle, shared: SharedState<'_>) -> Result<StateView, String> {
    let queue = shared.queue()?;
    let status = queue.status().map_err(|e| e.to_string())?;
    let setting = |key| queue.setting(key).ok().flatten();
    let game = shared.game_dir();
    let mut groups: Vec<Group> = Vec::new();
    for (kind, count) in queue.kinds_since(now() - 7 * 24 * 3600).map_err(|e| e.to_string())? {
        let label = kind_group(&kind);
        match groups.iter_mut().find(|g| g.label == label) {
            Some(group) => group.count += count,
            None => groups.push(Group { label, count }),
        }
    }
    groups.sort_by_key(|g| GROUP_ORDER.iter().position(|l| *l == g.label));
    let live = shared.live();
    Ok(StateView {
        version: rrc_core::VERSION,
        setup_done: setting(keys::SETUP_DONE).as_deref() == Some("1"),
        connected: shared.connected(),
        auto_upload: setting(keys::AUTO_UPLOAD).as_deref() == Some("1"),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        saves_found: game.as_deref().map(|g| rrc_core::game::saved_files(g).len()).unwrap_or(0),
        addon_version: game.as_deref().and_then(|g| addon::addon_version(&addon::installed_folder(g))),
        bundled_addon_version: addon::addon_version(&shared.addon_source),
        game_dir: game.map(|g| g.display().to_string()),
        wow_running: live.wow_running,
        working: live.working,
        last_error: live.last_error,
        observations: status.observations,
        pending: status.pending,
        last_upload_at: setting(keys::LAST_UPLOAD_AT).and_then(|v| v.parse().ok()),
        last_upload_count: setting(keys::LAST_UPLOAD_COUNT).and_then(|v| v.parse().ok()),
        this_week: groups,
    })
}

fn changed(app: &AppHandle) {
    let _ = app.emit("state-changed", ());
}

#[tauri::command]
pub async fn detect_game() -> Option<String> {
    addon::detect_game_dir().map(|p| p.display().to_string())
}

#[tauri::command]
pub async fn set_game_dir(app: AppHandle, shared: SharedState<'_>, path: String) -> Result<String, String> {
    let chosen = addon::normalize_game_dir(&PathBuf::from(&path)).ok_or(
        "That folder does not look like World of Warcraft. Choose the World of Warcraft folder or the _classic_beta_ folder inside it.",
    )?;
    let text = chosen.display().to_string();
    shared.queue()?.set_setting(keys::GAME_DIR, &text).map_err(|e| e.to_string())?;
    shared.nudge(Trigger::GameChanged);
    changed(&app);
    Ok(text)
}

#[tauri::command]
pub async fn install_addon(app: AppHandle, shared: SharedState<'_>) -> Result<String, String> {
    let game = shared.game_dir().ok_or("Choose the World of Warcraft folder first.")?;
    let version = addon::install(&shared.addon_source, &game)?;
    changed(&app);
    Ok(version)
}

#[tauri::command]
pub async fn pair(app: AppHandle, shared: SharedState<'_>, code: String) -> Result<(), String> {
    let name = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Windows PC".into());
    redeem_code(&Https::default(), shared.store.as_ref(), &code, &name).map_err(|e| e.to_string())?;
    shared.nudge(Trigger::SyncNow);
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn set_auto_upload(app: AppHandle, shared: SharedState<'_>, on: bool) -> Result<(), String> {
    shared.queue()?.set_setting(keys::AUTO_UPLOAD, if on { "1" } else { "0" }).map_err(|e| e.to_string())?;
    if on {
        shared.nudge(Trigger::SyncNow);
    }
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn set_autostart(app: AppHandle, on: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if on { manager.enable() } else { manager.disable() }.map_err(|e| e.to_string())?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn finish_setup(app: AppHandle, shared: SharedState<'_>) -> Result<(), String> {
    shared.queue()?.set_setting(keys::SETUP_DONE, "1").map_err(|e| e.to_string())?;
    shared.nudge(Trigger::SyncNow);
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn sync_now(shared: SharedState<'_>) -> Result<(), String> {
    shared.nudge(Trigger::SyncNow);
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordView {
    label: String,
    observed_at: Option<i64>,
    uploaded: bool,
    sent: String,
}

/// Newest records in plain words, with exactly what is (or will be) sent.
#[tauri::command]
pub async fn recent_records(shared: SharedState<'_>, limit: i64) -> Result<Vec<RecordView>, String> {
    let rows = shared.queue()?.recent(limit).map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let record: serde_json::Value = serde_json::from_str(&row.payload).unwrap_or_default();
            let kind = record.get("kind").and_then(|k| k.as_str()).unwrap_or("");
            RecordView {
                label: kind_label(kind),
                observed_at: record.get("observedAt").and_then(|t| t.as_i64()),
                uploaded: row.uploaded,
                sent: serde_json::to_string_pretty(&redact(&record)).unwrap_or_default(),
            }
        })
        .collect())
}

#[tauri::command]
pub async fn disconnect(app: AppHandle, shared: SharedState<'_>) -> Result<(), String> {
    shared.store.forget().map_err(|e| e.to_string())?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn forget_local_data(app: AppHandle, shared: SharedState<'_>) -> Result<(), String> {
    // Keep the connection and the game folder; only the records go.
    let mut queue = shared.queue()?;
    let game = queue.setting(keys::GAME_DIR).ok().flatten();
    queue.forget().map_err(|e| e.to_string())?;
    if let Some(game) = game {
        queue.set_setting(keys::GAME_DIR, &game).map_err(|e| e.to_string())?;
    }
    queue.set_setting(keys::SETUP_DONE, "1").map_err(|e| e.to_string())?;
    // Uploads start again only when the player turns them back on.
    queue.set_setting(keys::AUTO_UPLOAD, "0").map_err(|e| e.to_string())?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn open_page(app: AppHandle, page: String) -> Result<(), String> {
    let url = match page.as_str() {
        "account" => "https://restedrealm.com/account/collector",
        "privacy" => "https://restedrealm.com/privacy",
        "home" => "https://restedrealm.com/",
        _ => return Err("Unknown page".into()),
    };
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}
