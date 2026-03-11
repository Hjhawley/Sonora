//! core/db/mod.rs
//!
//! SQLite persistence layer.
//!
//! Rules:
//! - DB owns stable identity (TrackId).
//! - Path is the natural unique key until fingerprinting exists.
//! - 'present' = file currently discovered on disk
//! - 'hidden' = user removed it from Sonora view, but file still exists
//! - 'mtime' / 'size' prepare us for incremental scanning later

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::core::library::DiscoveredFile;
use crate::core::types::TrackId;

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

    fn init_schema(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA foreign_keys = ON;

                CREATE TABLE IF NOT EXISTS tracks (
                    id      INTEGER PRIMARY KEY,
                    path    TEXT NOT NULL UNIQUE
                );

                CREATE TABLE IF NOT EXISTS app_state (
                    key     TEXT PRIMARY KEY,
                    value   TEXT NOT NULL
                );
                "#,
            )
            .map_err(|e| e.to_string())?;

        self.ensure_column("tracks", "present", "INTEGER NOT NULL DEFAULT 1")?;
        self.ensure_column("tracks", "hidden", "INTEGER NOT NULL DEFAULT 0")?;
        self.ensure_column("tracks", "mtime", "INTEGER")?;
        self.ensure_column("tracks", "size", "INTEGER")?;

        Ok(())
    }

    fn ensure_column(&self, table: &str, column: &str, definition: &str) -> Result<(), String> {
        let pragma = format!("PRAGMA table_info({table})");
        let mut stmt = self.conn.prepare(&pragma).map_err(|e| e.to_string())?;
        let mut rows = stmt.query([]).map_err(|e| e.to_string())?;

        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let name: String = row.get(1).map_err(|e| e.to_string())?;
            if name == column {
                return Ok(());
            }
        }

        let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
        self.conn.execute(&sql, []).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Upsert all discovered files.
    ///
    /// Behavior:
    /// - Mark everything missing first ('present = 0')
    /// - For discovered files:
    ///   - INSERT OR IGNORE by path
    ///   - set 'present = 1'
    ///   - update mtime/size
    /// - preserve 'hidden'
    /// - return '(TrackId, PathBuf)' in the same order as discovered input
    pub fn upsert_discovered(
        &mut self,
        files: &[DiscoveredFile],
    ) -> Result<Vec<(TrackId, PathBuf)>, String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;

        tx.execute("UPDATE tracks SET present = 0", [])
            .map_err(|e| e.to_string())?;

        let mut out: Vec<(TrackId, PathBuf)> = Vec::with_capacity(files.len());

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

            let mut select = tx
                .prepare("SELECT id FROM tracks WHERE path = ?1")
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

                let id_i64: i64 = select
                    .query_row(params![p_str.as_ref()], |row| row.get(0))
                    .map_err(|e| e.to_string())?;

                out.push((TrackId(id_i64), f.path.clone()));
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(out)
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
    pub fn load_hidden_paths(&self) -> Result<Vec<(TrackId, PathBuf)>, String> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT id, path
                FROM tracks
                WHERE hidden = 1
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

    /// Future UI support: hide / unhide without touching the file
    pub fn set_hidden(&self, id: TrackId, hidden: bool) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE tracks SET hidden = ?2 WHERE id = ?1",
                params![id.0, if hidden { 1 } else { 0 }],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Load persisted master volume (0.0..=1.0), if present and valid
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

    /// Persist master volume (clamped to 0.0..=1.0)
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

pub fn default_db_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA not set".to_string())?;
        let mut dir = PathBuf::from(base);
        dir.push("Sonora");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        dir.push("sonora.sqlite3");
        return Ok(dir);
    }

    // macOS, not yet implemented
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").ok_or("HOME not set".to_string())?;
        let mut dir = PathBuf::from(home);
        dir.push("Library");
        dir.push("Application Support");
        dir.push("Sonora");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        dir.push("sonora.sqlite3");
        return Ok(dir);
    }

    // Linux, not yet implemented
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .or_else(|| {
                std::env::var_os("HOME").map(|h| {
                    let mut p = PathBuf::from(h);
                    p.push(".local");
                    p.push("share");
                    p.into_os_string()
                })
            })
            .ok_or("No HOME/XDG_DATA_HOME set".to_string())?;

        let mut dir = PathBuf::from(base);
        dir.push("sonora");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        dir.push("sonora.sqlite3");
        return Ok(dir);
    }
}
