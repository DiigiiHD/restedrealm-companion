//! Find the game folder and install the bundled addon into it.
//! Phase 4 replaces the simple folder guesses with Battle.net discovery.

use std::fs;
use std::path::{Path, PathBuf};

pub const ADDON_FOLDER: &str = "RestedRealmCollector";
pub const FOREVER_FOLDER: &str = "_classic_beta_";

fn looks_like_game_version(path: &Path) -> bool {
    path.join("WTF").is_dir() || path.join("Interface").is_dir()
}

/// Accept the Forever folder itself or the World of Warcraft folder above it.
pub fn normalize_game_dir(chosen: &Path) -> Option<PathBuf> {
    if chosen.join(FOREVER_FOLDER).is_dir() {
        return Some(chosen.join(FOREVER_FOLDER));
    }
    if chosen.file_name().is_some_and(|n| n == FOREVER_FOLDER) || looks_like_game_version(chosen) {
        return Some(chosen.to_path_buf());
    }
    None
}

/// The usual install locations, first match wins.
pub fn detect_game_dir() -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = ["PROGRAMFILES(X86)", "PROGRAMFILES", "PUBLIC"]
        .iter()
        .filter_map(|v| std::env::var_os(v).map(PathBuf::from))
        .collect();
    for drive in ["C", "D", "E", "F"] {
        roots.push(PathBuf::from(format!(r"{drive}:\")));
        roots.push(PathBuf::from(format!(r"{drive}:\Games")));
        roots.push(PathBuf::from(format!(r"{drive}:\Program Files (x86)")));
    }
    roots.into_iter().map(|root| root.join("World of Warcraft").join(FOREVER_FOLDER)).find(|p| p.is_dir())
}

/// The `## Version:` line of an addon folder's .toc, if any.
pub fn addon_version(folder: &Path) -> Option<String> {
    let toc = fs::read_to_string(folder.join(format!("{ADDON_FOLDER}.toc"))).ok()?;
    toc.lines().find_map(|line| line.strip_prefix("## Version:").map(|v| v.trim().to_string()))
}

pub fn installed_folder(game: &Path) -> PathBuf {
    game.join("Interface").join("AddOns").join(ADDON_FOLDER)
}

/// Copy the bundled addon's files over the installed ones. Other files in the
/// addon folder are left alone. Each file is written beside its target first.
pub fn install(source: &Path, game: &Path) -> Result<String, String> {
    let target = installed_folder(game);
    fs::create_dir_all(&target).map_err(|e| format!("Could not create the addon folder: {e}"))?;
    let entries = fs::read_dir(source).map_err(|e| format!("The bundled addon is missing: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let wanted = path.extension().is_some_and(|x| x == "lua" || x == "toc");
        if !path.is_file() || !wanted {
            continue;
        }
        let final_path = target.join(entry.file_name());
        let temp = target.join(format!(".{}.rrc-new", entry.file_name().to_string_lossy()));
        fs::copy(&path, &temp).map_err(|e| format!("Could not copy the addon: {e}"))?;
        fs::rename(&temp, &final_path).map_err(|e| format!("Could not replace the addon: {e}"))?;
    }
    addon_version(&target).ok_or_else(|| "The addon was copied but its version could not be read".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_and_accepts_either_folder() {
        let dir = tempfile_dir();
        let wow = dir.join("World of Warcraft");
        let forever = wow.join(FOREVER_FOLDER);
        fs::create_dir_all(forever.join("WTF")).unwrap();
        assert_eq!(normalize_game_dir(&wow), Some(forever.clone()));
        assert_eq!(normalize_game_dir(&forever), Some(forever.clone()));
        assert_eq!(normalize_game_dir(&dir.join("elsewhere")), None);

        let source = dir.join("bundle");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join(format!("{ADDON_FOLDER}.toc")), "## Title: x\n## Version: 9.9\n").unwrap();
        fs::write(source.join("Collector.lua"), "-- code").unwrap();
        fs::write(source.join("notes.txt"), "not copied").unwrap();
        fs::create_dir_all(installed_folder(&forever)).unwrap();
        fs::write(installed_folder(&forever).join("CompanionStatus.lua"), "kept").unwrap();
        assert_eq!(install(&source, &forever).unwrap(), "9.9");
        let installed = installed_folder(&forever);
        assert!(installed.join("Collector.lua").is_file());
        assert!(!installed.join("notes.txt").exists());
        assert_eq!(fs::read_to_string(installed.join("CompanionStatus.lua")).unwrap(), "kept");
        fs::remove_dir_all(dir).unwrap();
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rrc-addon-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
