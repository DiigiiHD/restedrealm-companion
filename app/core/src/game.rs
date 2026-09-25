//! Where the addon's saves are, and whether the game is running.

use crate::save::SAVE_FILE;
use std::path::{Path, PathBuf};

/// Executable names that mean WoW is open. Rollover waits while any is running.
pub const GAME_EXECUTABLES: &[&str] = &["WowB.exe", "Wow.exe", "WowClassic.exe", "WowClassicB.exe", "WowT.exe"];

/// Every account's addon save inside one game version folder, such as `_classic_beta_`.
pub fn saved_files(game: &Path) -> Vec<PathBuf> {
    let Ok(accounts) = std::fs::read_dir(game.join("WTF").join("Account")) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = accounts
        .flatten()
        .map(|entry| entry.path().join("SavedVariables").join(SAVE_FILE))
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    files
}

#[cfg(windows)]
pub fn wow_running() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    // SAFETY: standard Toolhelp snapshot walk; the handle is closed before returning.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            // Unknown means "assume running": rollover must never race the game.
            return true;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            if GAME_EXECUTABLES.iter().any(|exe| exe.eq_ignore_ascii_case(&name)) {
                found = true;
                break;
            }
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        found
    }
}

#[cfg(not(windows))]
pub fn wow_running() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_each_account_save() {
        let dir = tempfile::tempdir().unwrap();
        for account in ["B", "A"] {
            let saved = dir.path().join("WTF/Account").join(account).join("SavedVariables");
            std::fs::create_dir_all(&saved).unwrap();
            std::fs::write(saved.join(SAVE_FILE), "x").unwrap();
        }
        std::fs::create_dir_all(dir.path().join("WTF/Account/SavedVariables")).unwrap();
        let files = saved_files(dir.path());
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].parent().unwrap().parent().unwrap().file_name().unwrap(), "A");
        assert!(saved_files(&dir.path().join("missing")).is_empty());
    }
}
