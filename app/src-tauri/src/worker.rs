//! The background worker: notice completed saves, import them, upload, and free
//! the addon's space once the game is closed.

use crate::state::{keys, Shared, Trigger};
use notify::{RecursiveMode, Watcher};
use rrc_core::game::{saved_files, wow_running};
use rrc_core::queue::Queue;
use rrc_core::save::{compact_one, scan_one, source_id, SAVE_FILE};
use rrc_core::upload::{upload_all, Https};
use rrc_core::Error;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

/// A safety net in case a file notification is missed.
const TICK: Duration = Duration::from_secs(5 * 60);
/// Wait for the game to finish writing before reading.
const SETTLE: Duration = Duration::from_secs(3);
/// Only rewrite the game's save once this many records are safely queued, so
/// the file is touched rarely. The addon stops recording at 1,500.
pub const ROLLOVER_AT: i64 = 200;

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn watch(game: &Path, tx: Sender<Trigger>) -> Option<notify::RecommendedWatcher> {
    let wtf = game.join("WTF");
    if !wtf.is_dir() {
        return None;
    }
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event {
            if event.paths.iter().any(|p| p.file_name().is_some_and(|n| n == SAVE_FILE)) {
                let _ = tx.send(Trigger::SaveChanged);
            }
        }
    })
    .ok()?;
    watcher.watch(&wtf, RecursiveMode::Recursive).ok()?;
    Some(watcher)
}

pub fn run(app: AppHandle, shared: Arc<Shared>, rx: Receiver<Trigger>, tx: Sender<Trigger>) {
    let mut queue = match Queue::open(&shared.state_dir) {
        Ok(queue) => queue,
        Err(e) => {
            shared.live.lock().unwrap().last_error = Some(format!("The local queue could not be opened: {e}"));
            return;
        }
    };
    let mut watched: Option<PathBuf> = None;
    let mut _watcher = None;
    let mut forced = false;
    loop {
        let game = shared.game_dir();
        if game != watched {
            _watcher = game.as_deref().and_then(|g| watch(g, tx.clone()));
            watched = game.clone();
        }
        cycle(&app, &shared, &mut queue, game.as_deref(), forced);
        forced = false;
        match rx.recv_timeout(TICK) {
            Ok(Trigger::SyncNow) => forced = true,
            Ok(Trigger::GameChanged) => {}
            Ok(Trigger::SaveChanged) => {
                // Let a burst of file events settle into one import.
                loop {
                    match rx.recv_timeout(SETTLE) {
                        Ok(Trigger::SyncNow) => forced = true,
                        Ok(_) => {}
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn set_working(app: &AppHandle, shared: &Shared, working: bool) {
    shared.live.lock().unwrap().working = working;
    let _ = app.emit("state-changed", ());
}

fn cycle(app: &AppHandle, shared: &Shared, queue: &mut Queue, game: Option<&Path>, forced: bool) {
    set_working(app, shared, true);
    let running = wow_running();
    let mut problem: Option<String> = None;
    let saves = game.map(saved_files).unwrap_or_default();

    for path in &saves {
        match scan_one(queue, path, Duration::from_secs(1)) {
            Ok(_) => {}
            // The game is mid-write; the next notification brings it back.
            Err(Error::SaveFormat(reason)) if reason.contains("being written") || reason.contains("changed while") => {}
            // Never show an account folder name or save text.
            Err(e) => problem = Some(format!("One of the addon's saves could not be read: {e}")),
        }
    }

    let setup_done = queue.setting(keys::SETUP_DONE).ok().flatten().as_deref() == Some("1");

    // A new app version carries a new addon. Replace the installed one while
    // the game is closed; an addon the player removed is not put back.
    if let (true, false, Some(game)) = (setup_done, running, game) {
        let installed = crate::addon::addon_version(&crate::addon::installed_folder(game));
        let bundled = crate::addon::addon_version(&shared.addon_source);
        if installed.is_some() && bundled.is_some() && installed != bundled {
            if let Err(e) = crate::addon::install(&shared.addon_source, game) {
                problem = Some(format!("The addon could not be updated: {e}"));
            }
        }
    }
    let auto = queue.setting(keys::AUTO_UPLOAD).ok().flatten().as_deref() == Some("1");
    let pending = queue.status().map(|s| s.pending).unwrap_or(0);
    if setup_done && pending > 0 && (auto || forced) && shared.connected() {
        match upload_all(queue, &Https::default(), shared.store.as_ref()) {
            Ok(sent) if sent > 0 => {
                let _ = queue.set_setting(keys::LAST_UPLOAD_AT, &now().to_string());
                let _ = queue.set_setting(keys::LAST_UPLOAD_COUNT, &sent.to_string());
            }
            Ok(_) => {}
            Err(e) => problem = Some(e.to_string()),
        }
    }

    if setup_done && !running {
        for path in &saves {
            let Ok(source) = source_id(path) else { continue };
            if queue.imported_record_count(&source).ok().flatten().unwrap_or(0) < ROLLOVER_AT {
                continue;
            }
            match compact_one(queue, path, Duration::from_secs(1), &wow_running) {
                Ok(_) | Err(Error::Refused(_)) => {}
                Err(e) => problem = Some(format!("Freeing the addon's space failed: {e}")),
            }
        }
    }

    {
        let mut live = shared.live.lock().unwrap();
        live.last_error = problem;
        live.wow_running = running;
    }
    crate::tray::refresh(shared);
    set_working(app, shared, false);
}
