//! The spell IDs restedrealm.com checks in the background.
//!
//! The website publishes the list at `/api/collector/spell-scan`. The app
//! fetches it now and then and writes it into the addon folder as
//! `SpellScanList.lua`, which the game loads at login. That file is Lua the
//! game runs, so it is written only from validated numbers and short labels:
//! nothing the website sends can become code in the player's game.

use crate::{Error, Result};
use serde_json::Value as Json;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

pub const LIST_PATH: &str = "/api/collector/spell-scan";
pub const FILE_NAME: &str = "SpellScanList.lua";
const MAX_SPELLS: usize = 50_000;
const MAX_SPELL_ID: u64 = 10_000_000;
const MAX_PACK: u64 = 200;
const MAX_LIST_BYTES: u64 = 2_000_000;

#[derive(Debug, Clone, PartialEq)]
pub struct SpellList {
    pub list_version: String,
    pub build: String,
    pub pack_size: u64,
    pub spells: Vec<u64>,
}

fn invalid() -> Error {
    Error::Upload("The spell list from RestedRealm is not valid".into())
}

/// A version or build label: letters, digits, dots, dashes and underscores only.
fn label(value: Option<&Json>) -> Result<String> {
    let text = value.and_then(Json::as_str).ok_or_else(invalid)?;
    let ok = !text.is_empty()
        && text.len() <= 64
        && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_');
    if ok {
        Ok(text.to_string())
    } else {
        Err(invalid())
    }
}

pub fn parse(json: &Json) -> Result<SpellList> {
    let list_version = label(json.get("listVersion"))?;
    let build = label(json.get("build"))?;
    let pack_size = json.get("packSize").and_then(Json::as_u64).unwrap_or(MAX_PACK).clamp(1, MAX_PACK);
    let raw = json.get("spells").and_then(Json::as_array).ok_or_else(invalid)?;
    if raw.len() > MAX_SPELLS {
        return Err(invalid());
    }
    let mut spells = Vec::with_capacity(raw.len());
    for item in raw {
        match item.as_u64() {
            Some(id) if (1..=MAX_SPELL_ID).contains(&id) => spells.push(id),
            _ => return Err(invalid()),
        }
    }
    spells.sort_unstable();
    spells.dedup();
    Ok(SpellList { list_version, build, pack_size, spells })
}

/// The data file the addon loads. Only validated labels and numbers go in.
pub fn render_lua(list: &SpellList) -> String {
    let mut out = String::with_capacity(list.spells.len() * 7 + 400);
    out.push_str("-- Written by RestedRealm Companion: the spell IDs restedrealm.com checks in\n");
    out.push_str("-- the background. Data only, generated from validated numbers.\n");
    out.push_str("RestedRealmSpellScanList = {\n");
    out.push_str(&format!("  listVersion = \"{}\",\n", list.list_version));
    out.push_str(&format!("  build = \"{}\",\n", list.build));
    out.push_str(&format!("  packSize = {},\n", list.pack_size));
    out.push_str("  spells = {\n");
    for chunk in list.spells.chunks(20) {
        let line: Vec<String> = chunk.iter().map(u64::to_string).collect();
        out.push_str("    ");
        out.push_str(&line.join(", "));
        out.push_str(",\n");
    }
    out.push_str("  },\n}\n");
    out
}

/// The list version already written into the addon folder, if any.
pub fn written_version(addon_folder: &Path) -> Option<String> {
    let text = fs::read_to_string(addon_folder.join(FILE_NAME)).ok()?;
    let line = text.lines().find_map(|line| line.trim().strip_prefix("listVersion = \""))?;
    let value = line.strip_suffix("\",")?;
    label(Some(&Json::String(value.to_string()))).ok()
}

/// Write the file beside its target first, so the game never reads half a list.
pub fn write(addon_folder: &Path, list: &SpellList) -> Result<()> {
    let target = addon_folder.join(FILE_NAME);
    let temp = addon_folder.join(format!(".{FILE_NAME}.rrc-new"));
    fs::write(&temp, render_lua(list))?;
    fs::rename(&temp, &target)?;
    Ok(())
}

/// Fetch the list. No credential is sent and redirects are refused.
pub fn fetch(base_url: &str) -> Result<SpellList> {
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(30))
        .user_agent(&format!("RestedRealmCompanion/{}", crate::VERSION))
        .build();
    let url = format!("{}{}", base_url.trim_end_matches('/'), LIST_PATH);
    let response = match agent.get(&url).set("Accept", "application/json").call() {
        Ok(response) if response.status() == 200 => response,
        Ok(_) | Err(ureq::Error::Status(..)) => return Err(Error::Upload("RestedRealm has no spell list".into())),
        Err(ureq::Error::Transport(_)) => return Err(Error::Upload("Could not reach RestedRealm".into())),
    };
    let mut text = String::new();
    response.into_reader().take(MAX_LIST_BYTES + 1).read_to_string(&mut text)?;
    if text.len() as u64 > MAX_LIST_BYTES {
        return Err(invalid());
    }
    let json: Json = serde_json::from_str(&text).map_err(|_| invalid())?;
    parse(&json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Json {
        json!({ "build": "1.60.1.70205", "listVersion": "1.60.1.70205-7", "packSize": 200, "spells": [116, 17, 17, 120] })
    }

    #[test]
    fn a_valid_list_is_sorted_and_deduplicated() {
        let list = parse(&sample()).unwrap();
        assert_eq!(list.spells, vec![17, 116, 120]);
        assert_eq!(list.list_version, "1.60.1.70205-7");
        assert_eq!(list.pack_size, 200);
    }

    #[test]
    fn anything_that_could_become_code_is_refused() {
        for bad in [
            json!({ "build": "1", "listVersion": "x\", os.exit() --", "spells": [] }),
            json!({ "build": "1\n", "listVersion": "1", "spells": [] }),
            json!({ "build": "1", "listVersion": "", "spells": [] }),
            json!({ "build": "1", "listVersion": "1", "spells": ["17"] }),
            json!({ "build": "1", "listVersion": "1", "spells": [1.5] }),
            json!({ "build": "1", "listVersion": "1", "spells": [-3] }),
            json!({ "build": "1", "listVersion": "1", "spells": [0] }),
            json!({ "build": "1", "listVersion": "1", "spells": [10_000_001] }),
            json!({ "build": "1", "listVersion": "1" }),
        ] {
            assert!(parse(&bad).is_err(), "{bad}");
        }
        let too_many: Vec<u64> = (1..=50_001).collect();
        assert!(parse(&json!({ "build": "1", "listVersion": "1", "spells": too_many })).is_err());
    }

    #[test]
    fn pack_size_stays_within_what_the_website_takes() {
        let mut big = sample();
        big["packSize"] = json!(5000);
        assert_eq!(parse(&big).unwrap().pack_size, 200);
        big["packSize"] = json!(0);
        assert_eq!(parse(&big).unwrap().pack_size, 1);
    }

    #[test]
    fn the_written_file_holds_only_data() {
        let list = parse(&sample()).unwrap();
        let lua = render_lua(&list);
        for line in lua.lines() {
            let data_line =
                line.starts_with("--") || line.bytes().all(|b| b.is_ascii_alphanumeric() || b" .,-_=\"{}".contains(&b));
            assert!(data_line, "{line}");
        }
        assert!(lua.contains("listVersion = \"1.60.1.70205-7\","));
        assert!(lua.contains("    17, 116, 120,\n"));
    }

    #[test]
    fn the_written_version_reads_back() {
        let dir = std::env::temp_dir().join(format!("rrc-spells-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(written_version(&dir), None);
        write(&dir, &parse(&sample()).unwrap()).unwrap();
        assert_eq!(written_version(&dir).as_deref(), Some("1.60.1.70205-7"));
        // The placeholder the addon ships with has no version.
        fs::write(dir.join(FILE_NAME), "RestedRealmSpellScanList = nil\n").unwrap();
        assert_eq!(written_version(&dir), None);
        fs::remove_dir_all(&dir).unwrap();
    }
}
