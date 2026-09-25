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
