//! The private local queue. Same file and schema as the Python companion, so the
//! app opens a pilot queue as it is.

use crate::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

pub const QUEUE_FILE: &str = "queue.sqlite3";

pub struct Queue {
    pub db: Connection,
    pub state: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub observations: i64,
    pub pending: i64,
    pub sources: i64,
    pub last_import: Option<i64>,
    pub dropped: i64,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub seq: i64,
    pub digest: String,
    pub payload: String,
    pub queued_at: i64,
    pub uploaded: bool,
}

impl Queue {
    pub fn open(state: &Path) -> Result<Queue> {
        std::fs::create_dir_all(state)?;
        let db = Connection::open(state.join(QUEUE_FILE))?;
        // The app's window and its background worker each hold a connection.
        db.busy_timeout(std::time::Duration::from_secs(10))?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "FULL")?;
        db.pragma_update(None, "secure_delete", "ON")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS observations (
                source_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                digest TEXT NOT NULL,
                payload TEXT NOT NULL,
                queued_at INTEGER NOT NULL,
                uploaded_digest TEXT,
                PRIMARY KEY (source_id, seq)
            );
            CREATE TABLE IF NOT EXISTS imports (
                source_id TEXT PRIMARY KEY,
                save_digest TEXT NOT NULL,
                record_count INTEGER NOT NULL,
                dropped INTEGER NOT NULL,
                imported_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )?;
        let has_uploaded = db
            .prepare("PRAGMA table_info(observations)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .iter()
            .any(|name| name == "uploaded_digest");
        if !has_uploaded {
            db.execute("ALTER TABLE observations ADD COLUMN uploaded_digest TEXT", [])?;
        }
        Ok(Queue { db, state: state.to_path_buf() })
    }

    pub fn status(&self) -> Result<Status> {
        let observations = self.db.query_row("SELECT COUNT(*) FROM observations", [], |r| r.get(0))?;
        let pending = self.db.query_row(
            "SELECT COUNT(*) FROM observations WHERE uploaded_digest IS NULL OR uploaded_digest<>digest",
            [],
            |r| r.get(0),
        )?;
        let (sources, last, dropped): (i64, i64, i64) = self.db.query_row(
            "SELECT COUNT(*), COALESCE(MAX(imported_at), 0), COALESCE(SUM(dropped), 0) FROM imports",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        Ok(Status { observations, pending, sources, last_import: (last > 0).then_some(last), dropped })
    }

    /// Newest first, for the "View my data" screen.
    pub fn recent(&self, limit: i64) -> Result<Vec<Row>> {
        let mut statement = self.db.prepare(
            "SELECT seq, digest, payload, queued_at, uploaded_digest IS NOT NULL AND uploaded_digest=digest
             FROM observations ORDER BY queued_at DESC, seq DESC LIMIT ?",
        )?;
        let rows = statement
            .query_map([limit.clamp(1, 1000)], |r| {
                Ok(Row {
                    seq: r.get(0)?,
                    digest: r.get(1)?,
                    payload: r.get(2)?,
                    queued_at: r.get(3)?,
                    uploaded: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self.db.query_row("SELECT value FROM settings WHERE key=?", [key], |r| r.get(0)).optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.db.execute(
            "INSERT INTO settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Records the last import of one save held.
    pub fn imported_record_count(&self, source_id: &str) -> Result<Option<i64>> {
        Ok(self
            .db
            .query_row("SELECT record_count FROM imports WHERE source_id=?", [source_id], |r| r.get(0))
            .optional()?)
    }

    /// Queued records by kind whose `observedAt` is at or after `since`.
    pub fn kinds_since(&self, since: i64) -> Result<Vec<(String, i64)>> {
        let mut statement = self.db.prepare(
            "SELECT json_extract(payload, '$.kind'), COUNT(*) FROM observations
             WHERE json_extract(payload, '$.observedAt') >= ? GROUP BY 1 ORDER BY 2 DESC",
        )?;
        let rows = statement
            .query_map([since], |r| Ok((r.get::<_, Option<String>>(0)?.unwrap_or_default(), r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Delete the companion's copy only. The game save is not touched.
    pub fn forget(&mut self) -> Result<()> {
        let tx = self.db.transaction()?;
        tx.execute("DELETE FROM observations", [])?;
        tx.execute("DELETE FROM imports", [])?;
        tx.execute("DELETE FROM settings", [])?;
        tx.commit()?;
        self.db.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        self.db.execute("VACUUM", [])?;
        Ok(())
    }
}

/// Copy the Python companion's queue and backups into a new state folder, once.
/// The old folder is left untouched. Returns whether anything was copied.
pub fn migrate_legacy(old_state: &Path, new_state: &Path) -> Result<bool> {
    let old_queue = old_state.join(QUEUE_FILE);
    if new_state.join(QUEUE_FILE).exists() || !old_queue.is_file() {
        return Ok(false);
    }
    std::fs::create_dir_all(new_state)?;
    // SQLite's online backup reads a consistent copy, including a pending WAL.
    let source = Connection::open_with_flags(&old_queue, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let partial = new_state.join(format!("{QUEUE_FILE}.migrating"));
    let _ = std::fs::remove_file(&partial);
    {
        let mut target = Connection::open(&partial)?;
        rusqlite::backup::Backup::new(&source, &mut target)?.run_to_completion(256, std::time::Duration::ZERO, None)?;
    }
    let old_backups = old_state.join("Backups");
    if old_backups.is_dir() {
        let new_backups = new_state.join("Backups");
        std::fs::create_dir_all(&new_backups)?;
        for entry in std::fs::read_dir(&old_backups)?.flatten() {
            let target = new_backups.join(entry.file_name());
            if entry.path().is_file() && !target.exists() {
                std::fs::copy(entry.path(), target)?;
            }
        }
    }
    std::fs::rename(&partial, new_state.join(QUEUE_FILE))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::tests::{sample, Fixture};

    #[test]
    fn legacy_queue_is_copied_once_and_left_in_place() {
        let mut f = Fixture::new();
        f.write(&sample(1, "Pilot"));
        f.scan().unwrap();
        f.queue.set_setting("upload_opt_in", "1").unwrap();
        std::fs::create_dir_all(f.queue.state.join("Backups")).unwrap();
        std::fs::write(f.queue.state.join("Backups").join("a.lua"), "backup").unwrap();
        let new_state = f.dir.path().join("new");
        assert!(migrate_legacy(&f.queue.state, &new_state).unwrap());
        let migrated = Queue::open(&new_state).unwrap();
        assert_eq!(migrated.status().unwrap().observations, 1);
        assert_eq!(migrated.recent(1).unwrap()[0].digest, f.queue.recent(1).unwrap()[0].digest);
        assert!(new_state.join("Backups").join("a.lua").is_file());
        assert!(!migrate_legacy(&f.queue.state, &new_state).unwrap());
        assert_eq!(f.count(), 1);
    }

    #[test]
    fn kinds_since_counts_recent_records() {
        let mut f = Fixture::new();
        f.write(
            &sample(1, "x").replace("[\"kind\"] = \"gossip\",", "[\"kind\"] = \"gossip\", [\"observedAt\"] = 2000,"),
        );
        f.scan().unwrap();
        assert_eq!(f.queue.kinds_since(1000).unwrap(), vec![("gossip".to_string(), 1)]);
        assert!(f.queue.kinds_since(3000).unwrap().is_empty());
    }
}
