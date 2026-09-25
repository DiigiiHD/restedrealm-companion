//! What the window, the tray and the background worker share.

use rrc_core::credentials::CredentialStore;
use rrc_core::queue::Queue;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use tauri::menu::MenuItem;
use tauri::Wry;

/// Settings kept in the queue's settings table.
pub mod keys {
    pub const GAME_DIR: &str = "game_dir";
    pub const AUTO_UPLOAD: &str = "auto_upload";
    pub const SETUP_DONE: &str = "setup_done";
    /// The player's start-with-Windows choice, so it survives reinstalling.
    pub const AUTOSTART: &str = "autostart";
    pub const LAST_UPLOAD_AT: &str = "last_upload_at";
    pub const LAST_UPLOAD_COUNT: &str = "last_upload_count";
}

pub enum Trigger {
    /// A save file changed on disk.
    SaveChanged,
    /// The player asked to check and upload now.
    SyncNow,
    /// The game folder setting changed.
    GameChanged,
}

#[derive(Default, Clone)]
pub struct Live {
    pub working: bool,
    pub last_error: Option<String>,
    pub wow_running: bool,
    /// Set while a new version is being installed.
    pub update: Option<String>,
}

pub struct Shared {
    pub state_dir: PathBuf,
    pub addon_source: PathBuf,
    pub store: Box<dyn CredentialStore + Send + Sync>,
    pub live: Mutex<Live>,
    pub trigger: Mutex<Sender<Trigger>>,
    pub tray_status: Mutex<Option<MenuItem<Wry>>>,
    /// The browser connect request this app started, and when.
    pub pending_connect: Mutex<Option<(String, std::time::Instant)>>,
}

impl Shared {
    /// A short-lived connection for the window. The worker keeps its own.
    pub fn queue(&self) -> Result<Queue, String> {
        Queue::open(&self.state_dir).map_err(|e| e.to_string())
    }

    pub fn setting(&self, key: &str) -> Option<String> {
        self.queue().ok()?.setting(key).ok().flatten()
    }

    pub fn flag(&self, key: &str) -> bool {
        self.setting(key).as_deref() == Some("1")
    }

    pub fn game_dir(&self) -> Option<PathBuf> {
        self.setting(keys::GAME_DIR).filter(|s| !s.is_empty()).map(PathBuf::from)
    }

    pub fn connected(&self) -> bool {
        matches!(self.store.load(), Ok(Some(_)))
    }

    pub fn nudge(&self, trigger: Trigger) {
        let _ = self.trigger.lock().unwrap().send(trigger);
    }

    pub fn live(&self) -> Live {
        self.live.lock().unwrap().clone()
    }
}
