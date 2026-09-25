//! Import completed saves into the queue, and free the addon's space afterwards.

use crate::canon::{lua_to_canonical, sha256_hex};
use crate::lua::{parse_save, parse_save_with_span, Value};
use crate::queue::Queue;
use crate::{Error, Result};
use rusqlite::{params, OptionalExtension};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A full addon save (5,000 records with quest text) stays far below this.
pub const MAX_SAVE_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_RECORDS: usize = 50_000;
pub const FOREVER_PRODUCT: &str = "wow_classic_beta";
pub const SAVE_FILE: &str = "RestedRealmCollector.lua";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanResult {
    pub new: usize,
    pub updated: usize,
    pub records: Option<usize>,
    pub dropped: Option<i64>,
    pub unchanged: bool,
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn fingerprint(path: &Path) -> Result<(u64, Option<SystemTime>)> {
    let meta = fs::metadata(path)?;
    Ok((meta.len(), meta.modified().ok()))
}

/// Read a save only once its size and time have settled, and only if it did not
/// change while it was read.
pub fn stable_bytes(path: &Path, delay: Duration) -> Result<Vec<u8>> {
    let first = fingerprint(path)?;
    if first.0 > MAX_SAVE_BYTES {
        return Err(Error::SaveFormat("save exceeds 128 MiB limit".into()));
    }
    std::thread::sleep(delay);
    let second = fingerprint(path)?;
    if first != second {
        return Err(Error::SaveFormat("save is still being written".into()));
    }
    let payload = fs::read(path)?;
    let third = fingerprint(path)?;
    if second != third || payload.len() as u64 != second.0 {
        return Err(Error::SaveFormat("save changed while reading".into()));
    }
    Ok(payload)
}

fn decode(payload: &[u8]) -> Result<&str> {
    let body = payload.strip_prefix(b"\xef\xbb\xbf").unwrap_or(payload);
    std::str::from_utf8(body).map_err(|_| Error::SaveFormat("save is not valid UTF-8".into()))
}

/// Python's `str.casefold()` for the characters a Windows path can realistically hold.
fn casefold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            'ß' | 'ẞ' => out.push_str("ss"),
            'ς' => out.push('σ'),
            _ => out.extend(ch.to_lowercase()),
        }
    }
    out
}

/// Python's `Path.resolve()` text: the real path without Windows' `\\?\` prefix.
fn resolved_text(path: &Path) -> Result<String> {
    let real = fs::canonicalize(path)?;
    let text = real.to_string_lossy().into_owned();
    Ok(if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        text
    })
}

/// Stable private identity of one save file: a hash of its real path.
pub fn source_id(path: &Path) -> Result<String> {
    Ok(sha256_hex(casefold(&resolved_text(path)?).as_bytes()))
}

pub fn validate_record(record: &Value) -> Result<i64> {
    if !matches!(record, Value::Map(_)) {
        return Err(Error::SaveFormat("observation must be a table".into()));
    }
    let seq = match record.get("seq") {
        Some(Value::Int(seq)) if *seq >= 1 => *seq,
        _ => return Err(Error::SaveFormat("observation has no valid sequence".into())),
    };
    match record.get("kind") {
        Some(Value::Str(kind)) if kind.chars().count() <= 80 => {}
        _ => return Err(Error::SaveFormat("observation has no valid kind".into())),
    }
    match record.get("context") {
        Some(context @ Value::Map(_)) if context.get("product") == Some(&Value::Str(FOREVER_PRODUCT.into())) => {}
        _ => return Err(Error::SaveFormat("observation product is not Forever".into())),
    }
    if !matches!(record.get("data"), Some(Value::Map(_))) {
        return Err(Error::SaveFormat("observation data must be a table".into()));
    }
    Ok(seq)
}

fn records_of(db: &Value) -> Result<&[Value]> {
    match db.get("records") {
        None => Ok(&[]),
        Some(v) if v.is_empty_map() => Ok(&[]),
        Some(Value::List(items)) if items.len() <= MAX_RECORDS => Ok(items),
        _ => Err(Error::SaveFormat("invalid observations list".into())),
    }
}

/// Canonical text and digest of a record, as the Python companion computes them.
pub fn record_digest(record: &Value) -> Result<(String, String)> {
    let canonical = lua_to_canonical(record)?;
    let digest = sha256_hex(canonical.as_bytes());
    Ok((canonical, digest))
}

/// Copy one completed save into the queue in a single transaction. A partial or
/// damaged save leaves the queue exactly as it was.
pub fn scan_one(queue: &mut Queue, path: &Path, delay: Duration) -> Result<ScanResult> {
    let payload = stable_bytes(path, delay)?;
    let save_digest = sha256_hex(&payload);
    let source = source_id(path)?;
    let prior: Option<String> =
        queue.db.query_row("SELECT save_digest FROM imports WHERE source_id=?", [&source], |r| r.get(0)).optional()?;
    if prior.as_deref() == Some(save_digest.as_str()) {
        return Ok(ScanResult { new: 0, updated: 0, records: None, dropped: None, unchanged: true });
    }
    let db = parse_save(decode(&payload)?)?;
    let records = records_of(&db)?;
    let mut prepared = Vec::with_capacity(records.len());
    let mut seen = std::collections::HashSet::new();
    for record in records {
        let seq = validate_record(record)?;
        if !seen.insert(seq) {
            return Err(Error::SaveFormat("duplicate sequence in save".into()));
        }
        let (canonical, digest) = record_digest(record)?;
        prepared.push((seq, digest, canonical));
    }
    let dropped = match db.get("dropped") {
        Some(Value::Int(n)) if *n >= 0 => *n,
        _ => 0,
    };
    let stamp = now();
    let (mut new, mut updated) = (0, 0);
    let tx = queue.db.transaction()?;
    for (seq, digest, canonical) in &prepared {
        let old: Option<String> = tx
            .query_row("SELECT digest FROM observations WHERE source_id=? AND seq=?", params![source, seq], |r| {
                r.get(0)
            })
            .optional()?;
        match old {
            None => {
                tx.execute(
                    "INSERT INTO observations(source_id,seq,digest,payload,queued_at) VALUES (?, ?, ?, ?, ?)",
                    params![source, seq, digest, canonical, stamp],
                )?;
                new += 1;
            }
            Some(old) if &old != digest => {
                // The addon can correct old observations during a schema migration.
                tx.execute(
                    "UPDATE observations SET digest=?, payload=?, uploaded_digest=NULL WHERE source_id=? AND seq=?",
                    params![digest, canonical, source, seq],
                )?;
                updated += 1;
            }
            Some(_) => {}
        }
    }
    tx.execute(
        "INSERT INTO imports VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(source_id) DO UPDATE SET save_digest=excluded.save_digest,
         record_count=excluded.record_count, dropped=excluded.dropped, imported_at=excluded.imported_at",
        params![source, save_digest, records.len() as i64, dropped, stamp],
    )?;
    tx.commit()?;
    Ok(ScanResult { new, updated, records: Some(records.len()), dropped: Some(dropped), unchanged: false })
}

fn fsync_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

/// Remove already-queued records from a closed game's save, keeping every other
/// field byte for byte and a private backup of the original.
pub fn compact_one(queue: &Queue, path: &Path, delay: Duration, game_running: &dyn Fn() -> bool) -> Result<usize> {
    let changed = || Error::Refused("WoW started or the save changed; nothing was removed".into());
    if game_running() {
        return Err(Error::Refused("Close WoW completely before freeing the addon's space".into()));
    }
    let original = stable_bytes(path, delay)?;
    let text = decode(&original)?;
    let parsed = parse_save_with_span(text)?;
    let records = records_of(&parsed.db)?;
    if records.is_empty() {
        return Ok(0);
    }
    let Some((start, end)) = parsed.records_span else {
        return Err(Error::SaveFormat("no compactable observation list".into()));
    };
    let source = source_id(path)?;
    for record in records {
        let seq = validate_record(record)?;
        let (_, digest) = record_digest(record)?;
        let queued: Option<String> = queue
            .db
            .query_row("SELECT digest FROM observations WHERE source_id=? AND seq=?", params![source, seq], |r| {
                r.get(0)
            })
            .optional()?;
        if queued.as_deref() != Some(digest.as_str()) {
            return Err(Error::Refused("Some game observations are not yet safely queued; scan again first".into()));
        }
    }
    let replacement = format!("{}{{}}{}", &text[..start], &text[end..]);
    let check = parse_save(&replacement)?;
    if !check.get("records").is_some_and(Value::is_empty_map) {
        return Err(Error::Refused("rollover verification failed".into()));
    }
    if game_running() || stable_bytes(path, Duration::ZERO)? != original {
        return Err(changed());
    }

    let backups = queue.state.join("Backups");
    fs::create_dir_all(&backups)?;
    let original_digest = sha256_hex(&original);
    let backup = backups.join(format!("{original_digest}.lua"));
    if backup.exists() {
        if sha256_hex(&fs::read(&backup)?) != original_digest {
            return Err(Error::Refused("existing backup does not match the game save".into()));
        }
    } else {
        fsync_write(&backup, &original)?;
    }

    let parent = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    let mut temp = tempfile::Builder::new().prefix(".rrc-").suffix(".tmp").tempfile_in(parent)?;
    if original.starts_with(b"\xef\xbb\xbf") {
        temp.write_all(b"\xef\xbb\xbf")?;
    }
    temp.write_all(replacement.as_bytes())?;
    temp.as_file().sync_all()?;
    if game_running() || stable_bytes(path, Duration::ZERO)? != original {
        return Err(changed());
    }
    temp.persist(path).map_err(|e| Error::Io(e.error))?;
    Ok(records.len())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn sample(seq: i64, title: &str) -> String {
        format!(
            "RestedRealmCollectorDB = {{\n [\"schema\"] = 1, [\"dropped\"] = 0, [\"records\"] = {{\n  {{ [\"seq\"] = {seq}, [\"kind\"] = \"gossip\",\n    [\"context\"] = {{ [\"product\"] = \"wow_classic_beta\", [\"build\"] = \"70009\", [\"locale\"] = \"enUS\" }},\n    [\"data\"] = {{ [\"title\"] = \"{title}\" }} }},\n }},\n}}\n"
        )
    }

    pub struct Fixture {
        pub dir: tempfile::TempDir,
        pub path: PathBuf,
        pub queue: Queue,
    }

    impl Fixture {
        pub fn new() -> Fixture {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(SAVE_FILE);
            let queue = Queue::open(&dir.path().join("state")).unwrap();
            Fixture { dir, path, queue }
        }

        pub fn write(&self, text: &str) {
            fs::write(&self.path, text).unwrap();
        }

        pub fn scan(&mut self) -> Result<ScanResult> {
            scan_one(&mut self.queue, &self.path, Duration::ZERO)
        }

        pub fn count(&self) -> i64 {
            self.queue.status().unwrap().observations
        }
    }

    #[test]
    fn repeat_restart_and_corrected_save() {
        let mut f = Fixture::new();
        f.write(&sample(1, "First"));
        assert_eq!(f.scan().unwrap().new, 1);
        assert!(f.scan().unwrap().unchanged);
        f.queue = Queue::open(&f.dir.path().join("state")).unwrap();
        assert_eq!(f.count(), 1);
        f.write(&sample(1, "Corrected"));
        assert_eq!(f.scan().unwrap().updated, 1);
        assert_eq!(f.count(), 1);
        assert!(f.queue.recent(1).unwrap()[0].payload.contains("Corrected"));
    }

    #[test]
    fn incomplete_save_does_not_change_queue() {
        let mut f = Fixture::new();
        f.write(&sample(1, "First"));
        f.scan().unwrap();
        f.write("RestedRealmCollectorDB = { [\"records\"] = {");
        assert!(matches!(f.scan(), Err(Error::SaveFormat(_))));
        assert_eq!(f.count(), 1);
    }

    #[test]
    fn new_record_preserves_old_one() {
        let mut f = Fixture::new();
        f.write(&sample(1, "First"));
        f.scan().unwrap();
        f.write(&sample(2, "First"));
        assert_eq!(f.scan().unwrap().new, 1);
        assert_eq!(f.count(), 2);
    }

    #[test]
    fn foreign_product_is_rejected() {
        let mut f = Fixture::new();
        f.write(&sample(1, "First").replace("wow_classic_beta", "wow"));
        assert!(f.scan().is_err());
        assert_eq!(f.count(), 0);
    }

    #[test]
    fn rollover_only_after_archiving_and_game_exit() {
        let mut f = Fixture::new();
        f.write(&sample(1, "First"));
        assert!(matches!(compact_one(&f.queue, &f.path, Duration::ZERO, &|| true), Err(Error::Refused(_))));
        assert!(matches!(compact_one(&f.queue, &f.path, Duration::ZERO, &|| false), Err(Error::Refused(_))));
        f.scan().unwrap();
        assert_eq!(compact_one(&f.queue, &f.path, Duration::ZERO, &|| false).unwrap(), 1);
        let after = parse_save(&fs::read_to_string(&f.path).unwrap()).unwrap();
        assert!(after.get("records").unwrap().is_empty_map());
        assert_eq!(after.get("schema"), Some(&Value::Int(1)));
        assert_eq!(f.count(), 1);
        assert_eq!(fs::read_dir(f.queue.state.join("Backups")).unwrap().count(), 1);
    }

    /// A full addon save: 5,000 records with quest text, items and objectives.
    fn full_save() -> String {
        let text = "Greetings, <name>. ".repeat(55);
        let mut out = String::from("RestedRealmCollectorDB = {\n [\"dropped\"] = 0, [\"records\"] = {\n");
        for seq in 1..=5000 {
            out.push_str(&format!(
                "  {{ [\"seq\"] = {seq}, [\"kind\"] = \"quest\", [\"textSchema\"] = 1, [\"observedAt\"] = 1790000000, \
                 [\"context\"] = {{ [\"product\"] = \"wow_classic_beta\", [\"build\"] = \"70009\", [\"locale\"] = \"enUS\", \
                 [\"location\"] = {{ [\"mapID\"] = 1420, [\"x\"] = 0.4512, [\"y\"] = 0.6634, [\"zone\"] = \"Tirisfal Glades\" }} }}, \
                 [\"data\"] = {{ [\"id\"] = {seq}, [\"title\"] = \"Quest {seq}\", [\"questText\"] = \"{text}\", \
                 [\"items\"] = {{ {items} }}, [\"objectives\"] = {{ {objectives} }} }} }},\n",
                items = (0..10).map(|i| format!("{{ [\"type\"] = \"choice\", [\"id\"] = {}, [\"quantity\"] = 1 }}", 3000 + i)).collect::<Vec<_>>().join(", "),
                objectives = (0..4).map(|i| format!("{{ [\"type\"] = \"monster\", [\"required\"] = {i}, [\"finished\"] = false }}")).collect::<Vec<_>>().join(", "),
            ));
        }
        out.push_str(" },\n}\n");
        out
    }

    #[test]
    fn a_full_5000_record_save_is_read_and_freed() {
        let mut f = Fixture::new();
        let save = full_save();
        assert!((save.len() as u64) < MAX_SAVE_BYTES / 4, "a full save is {} bytes", save.len());
        f.write(&save);
        let started = std::time::Instant::now();
        let result = f.scan().unwrap();
        let took = started.elapsed();
        assert_eq!(result.new, 5000);
        eprintln!("full save: {} MB, imported in {:?}", save.len() / 1_000_000, took);
        assert_eq!(compact_one(&f.queue, &f.path, Duration::ZERO, &|| false).unwrap(), 5000);
    }

    #[test]
    fn forget_queue_clears_settings_and_preserves_game_save() {
        let mut f = Fixture::new();
        f.write(&sample(1, "First"));
        f.scan().unwrap();
        f.queue.set_setting("upload_opt_in", "1").unwrap();
        f.queue.forget().unwrap();
        assert_eq!(f.count(), 0);
        assert_eq!(f.queue.setting("upload_opt_in").unwrap(), None);
        assert!(fs::read_to_string(&f.path).unwrap().contains("\"seq\"] = 1"));
    }
}
