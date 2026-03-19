//! core/db/mod.rs
//!
//! SQLite persistence layer.
//!
//! Rules:
//! - DB owns stable identity (TrackId).
//! - Path is the natural unique key until fingerprinting exists.
//! - `present` = filesystem fact from the latest scan.
//! - `hidden` = user intent to ignore a file in normal Sonora views.
//! - Missing is represented as `present = 0`.
//! - `mtime` / `size` prepare us for incremental scanning later.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::core::library::DiscoveredFile;
use crate::core::types::TrackId;

mod paths;
mod schema;

pub use paths::default_db_path;

const APP_STATE_VOLUME_KEY: &str = "volume";

pub struct Db {
    conn: Connection,
}

impl Db {
    /// Open (or create) the DB file and ensure schema exists.
    pub fn open(db_path: &Path) -> Result<Self, String> {
        let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }
}

impl Db {
    /// Reconcile DB state against the latest discovered filesystem set.
    ///
    /// Behavior:
    /// - Mark everything missing first (`present = 0`)
    /// - For discovered files:
    ///   - INSERT OR IGNORE by path
    ///   - set `present = 1`
    ///   - update mtime/size
    /// - preserve `hidden`
    ///
    /// Important:
    /// This function updates DB truth only. UI lists should be loaded from
    /// `load_visible_paths()`, `load_hidden_paths()`, or `load_missing_paths()`
    /// after reconciliation, rather than from the discovered set directly.
    pub fn upsert_discovered(&mut self, files: &[DiscoveredFile]) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;

        tx.execute("UPDATE tracks SET present = 0", [])
            .map_err(|e| e.to_string())?;

        {
            let mut insert = tx
                .prepare("INSERT OR IGNORE INTO tracks(path) VALUES (?1)")
                .map_err(|e| e.to_string())?;

            let mut update = tx
                .prepare(
                    "UPDATE tracks
                     SET present = 1, mtime = ?2, size = ?3
                     WHERE path = ?1",
                )
                .map_err(|e| e.to_string())?;

            for f in files {
                let p_str = f.path.to_string_lossy();

                insert
                    .execute(params![p_str.as_ref()])
                    .map_err(|e| e.to_string())?;

                update
                    .execute(params![
                        p_str.as_ref(),
                        f.mtime_unix,
                        f.size.map(|s| s as i64)
                    ])
                    .map_err(|e| e.to_string())?;
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Load currently visible library rows:
    /// - present on disk
    /// - not hidden by the user
    pub fn load_visible_paths(&self) -> Result<Vec<(TrackId, PathBuf)>, String> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT id, path
                FROM tracks
                WHERE present = 1 AND hidden = 0
                ORDER BY path
                "#,
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                Ok((TrackId(id), PathBuf::from(path)))
            })
            .map_err(|e| e.to_string())?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// Future UI support: hidden list.
    ///
    /// Hidden means:
    /// - file is still present on disk
    /// - user intentionally removed it from normal Sonora views
    pub fn load_hidden_paths(&self) -> Result<Vec<(TrackId, PathBuf)>, String> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT id, path
                FROM tracks
                WHERE present = 1 AND hidden = 1
                ORDER BY path
                "#,
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                Ok((TrackId(id), PathBuf::from(path)))
            })
            .map_err(|e| e.to_string())?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// Future UI support: missing list.
    ///
    /// Missing means:
    /// - Sonora previously knew about this path
    /// - latest scan did not find a file there
    ///
    /// This includes rows that were hidden before they went missing.
    pub fn load_missing_paths(&self) -> Result<Vec<(TrackId, PathBuf)>, String> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT id, path
                FROM tracks
                WHERE present = 0
                ORDER BY path
                "#,
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                Ok((TrackId(id), PathBuf::from(path)))
            })
            .map_err(|e| e.to_string())?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// Hide / unhide without touching the file or its presence state.
    pub fn set_hidden(&self, id: TrackId, hidden: bool) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE tracks SET hidden = ?2 WHERE id = ?1",
                params![id.0, if hidden { 1 } else { 0 }],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Permanently delete a track record from Sonora's DB.
    ///
    /// This does NOT touch the actual file on disk.
    pub fn delete_track(&self, id: TrackId) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM tracks WHERE id = ?1", params![id.0])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl Db {
    /// Load persisted master volume (0.0..=1.0), if present and valid.
    pub fn load_volume(&self) -> Result<Option<f32>, String> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM app_state WHERE key = ?1",
                params![APP_STATE_VOLUME_KEY],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        let Some(raw) = raw else {
            return Ok(None);
        };

        let parsed = raw.parse::<f32>().map_err(|e| e.to_string())?;
        Ok(Some(parsed.clamp(0.0, 1.0)))
    }

    /// Persist master volume (clamped to 0.0..=1.0).
    pub fn save_volume(&self, volume: f32) -> Result<(), String> {
        let volume = volume.clamp(0.0, 1.0);

        self.conn
            .execute(
                r#"
                INSERT INTO app_state(key, value)
                VALUES (?1, ?2)
                ON CONFLICT(key) DO UPDATE SET value = excluded.value
                "#,
                params![APP_STATE_VOLUME_KEY, volume.to_string()],
            )
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}
