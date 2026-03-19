//! core/db/mod.rs
//!
//! SQLite persistence layer.
//!
//! Rules:
//! - DB owns stable identity (TrackId).
//! - Path is the natural unique key until fingerprinting exists.
//! - `present` = filesystem fact from the latest relevant scan.
//! - `hidden` = user intent to ignore a file in normal Sonora views.
//! - Missing is represented as `present = 0`.
//! - `mtime` / `size` prepare us for incremental scanning later.
//! - Library roots are persistent DB-backed configuration.

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
    /// Load enabled persistent library roots.
    pub fn load_roots(&self) -> Result<Vec<PathBuf>, String> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT path
                FROM library_roots
                WHERE enabled = 1
                ORDER BY path
                "#,
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                let path: String = row.get(0)?;
                Ok(PathBuf::from(path))
            })
            .map_err(|e| e.to_string())?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    /// Add a persistent library root if it does not already exist.
    pub fn add_root(&self, root: &Path) -> Result<(), String> {
        let root_str = root.to_string_lossy();

        self.conn
            .execute(
                r#"
                INSERT INTO library_roots(path, enabled)
                VALUES (?1, 1)
                ON CONFLICT(path) DO UPDATE SET enabled = 1
                "#,
                params![root_str.as_ref()],
            )
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Remove a persistent library root from the DB.
    pub fn remove_root(&self, root: &Path) -> Result<(), String> {
        let root_str = root.to_string_lossy();

        self.conn
            .execute(
                "DELETE FROM library_roots WHERE path = ?1",
                params![root_str.as_ref()],
            )
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Delete track records under `removed_root`, but keep any track whose path
    /// is still covered by at least one remaining root.
    ///
    /// Example:
    /// - removed_root = D:\Music
    /// - remaining_roots includes D:\Music\Soundtracks
    ///
    /// Then:
    /// - D:\Music\Pink Floyd\Time.mp3 -> deleted
    /// - D:\Music\Soundtracks\FF7\Prelude.mp3 -> kept
    pub fn delete_uncovered_tracks_under_root(
        &self,
        removed_root: &Path,
        remaining_roots: &[PathBuf],
    ) -> Result<usize, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path FROM tracks")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                Ok((TrackId(id), PathBuf::from(path)))
            })
            .map_err(|e| e.to_string())?;

        let mut ids_to_delete: Vec<TrackId> = Vec::new();

        for row in rows {
            let (id, path) = row.map_err(|e| e.to_string())?;

            // Only consider tracks that were under the root being removed.
            if !path.starts_with(removed_root) {
                continue;
            }

            // Keep the track if any remaining root still covers it.
            let still_covered = remaining_roots.iter().any(|root| path.starts_with(root));

            if !still_covered {
                ids_to_delete.push(id);
            }
        }

        if ids_to_delete.is_empty() {
            return Ok(0);
        }

        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        {
            let mut delete = tx
                .prepare("DELETE FROM tracks WHERE id = ?1")
                .map_err(|e| e.to_string())?;

            for id in &ids_to_delete {
                delete.execute(params![id.0]).map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;

        Ok(ids_to_delete.len())
    }
}

impl Db {
    /// Reconcile DB state against the latest discovered filesystem set for a
    /// specific set of scanned roots.
    ///
    /// Behavior:
    /// - Only tracks under the scanned roots are eligible to be marked missing.
    /// - For discovered files:
    ///   - INSERT OR IGNORE by path
    ///   - set `present = 1`
    ///   - update `mtime` / `size`
    /// - preserve `hidden`
    ///
    /// Important:
    /// This function updates DB truth only for the scanned roots. It does NOT
    /// globally mark unrelated library entries missing.
    pub fn upsert_discovered(
        &mut self,
        scanned_roots: &[PathBuf],
        files: &[DiscoveredFile],
    ) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;

        // Find all tracked rows that belong to the roots we are scanning,
        // and mark only those rows missing up front.
        let mut ids_under_scanned_roots: Vec<TrackId> = Vec::new();
        {
            let mut stmt = tx
                .prepare("SELECT id, path FROM tracks")
                .map_err(|e| e.to_string())?;

            let rows = stmt
                .query_map([], |row| {
                    let id: i64 = row.get(0)?;
                    let path: String = row.get(1)?;
                    Ok((TrackId(id), PathBuf::from(path)))
                })
                .map_err(|e| e.to_string())?;

            for row in rows {
                let (id, path) = row.map_err(|e| e.to_string())?;
                if scanned_roots.iter().any(|root| path.starts_with(root)) {
                    ids_under_scanned_roots.push(id);
                }
            }
        }

        if !ids_under_scanned_roots.is_empty() {
            let mut mark_missing = tx
                .prepare("UPDATE tracks SET present = 0 WHERE id = ?1")
                .map_err(|e| e.to_string())?;

            for id in &ids_under_scanned_roots {
                mark_missing
                    .execute(params![id.0])
                    .map_err(|e| e.to_string())?;
            }
        }

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

    /// Missing means:
    /// - Sonora previously knew about this path
    /// - latest relevant scan did not find a file there
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
