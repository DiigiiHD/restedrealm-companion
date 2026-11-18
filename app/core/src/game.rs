//! Where the addon's saves are, and whether the game is running.

use crate::save::SAVE_FILE;
use std::path::{Path, PathBuf};

/// Executable names that mean WoW is open. Rollover waits while any is running.
pub const GAME_EXECUTABLES: &[&str] = &["WowB.exe", "Wow.exe", "WowClassic.exe", "WowClassicB.exe", "WowT.exe"];

/// Whether a process name is a game client. Besides the known names, any
/// `Wow*.exe` counts, so a client name Blizzard introduces (the released
/// Forever's included) also holds back rollover. Wrongly thinking the game is
/// open only delays emptying the save; the opposite could race the game.
pub fn is_game_executable(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    GAME_EXECUTABLES.iter().any(|exe| exe.eq_ignore_ascii_case(name))
        || (lower.starts_with("wow") && lower.ends_with(".exe"))
}

/// Every account's addon save inside one game version folder, such as `_forever_`.
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

/// Whether a running process is the game. The name alone is not enough: tools
/// like WowUp (`WowUp-CF.exe`) also start with "Wow" and run all day, which
/// held back rollover for weeks. A game client sits in its version folder
/// beside `WTF` (or below the install's `.build.info`). A path that cannot be
/// read counts as the game, so rollover never races it.
pub fn is_game_process(name: &str, path: Option<&Path>) -> bool {
    if !is_game_executable(name) {
        return false;
    }
    let Some(folder) = path.and_then(Path::parent) else { return true };
    folder.join("WTF").is_dir() || folder.parent().is_some_and(|root| root.join(".build.info").is_file())
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
            // Name first: only open processes that could be the game.
            if is_game_executable(&name) && is_game_process(&name, image_path(entry.th32ProcessID).as_deref()) {
                found = true;
                break;
            }
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        found
    }
}

/// The full path of a process's executable, if Windows lets us read it.
#[cfg(windows)]
fn image_path(pid: u32) -> Option<PathBuf> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: the handle is checked and closed; the buffer length is passed in and updated.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut len) != 0;
        CloseHandle(handle);
        ok.then(|| PathBuf::from(String::from_utf16_lossy(&buffer[..len as usize])))
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
    fn any_wow_client_holds_back_rollover() {
        for name in ["Wow.exe", "WowB.exe", "wowclassic.exe", "WowF.exe", "WowForever.exe"] {
            assert!(is_game_executable(name), "{name}");
        }
        for name in ["Battle.net.exe", "Agent.exe", "wow", "notwow.exe", "explorer.exe"] {
            assert!(!is_game_executable(name), "{name}");
        }
    }

    #[test]
    fn addon_managers_named_wow_do_not_hold_back_rollover() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("World of Warcraft/_forever_");
        std::fs::create_dir_all(game.join("WTF")).unwrap();
        let other = dir.path().join("Programs/wowup-cf");
        std::fs::create_dir_all(&other).unwrap();
        assert!(is_game_process("Wow.exe", Some(&game.join("Wow.exe"))));
        assert!(!is_game_process("WowUp-CF.exe", Some(&other.join("WowUp-CF.exe"))));
        // Not yet launched once (no WTF), but inside a Blizzard install.
        let fresh = dir.path().join("World of Warcraft/_classic_");
        std::fs::create_dir_all(&fresh).unwrap();
        std::fs::write(dir.path().join("World of Warcraft/.build.info"), "x").unwrap();
        assert!(is_game_process("WowClassic.exe", Some(&fresh.join("WowClassic.exe"))));
        // Unknown path: assume the game, never race it.
        assert!(is_game_process("WowUp-CF.exe", None));
        assert!(!is_game_process("explorer.exe", None));
    }

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
