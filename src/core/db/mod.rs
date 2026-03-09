//! core/db/mod.rs
//!
//! SQLite persistence layer (MVP).
//!
//! Rules:
//! - DB owns stable identity (TrackId).
//! - Path is the natural unique key pre-fingerprinting.
//! - We keep the schema minimal and expand later.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};

use crate::core::types::TrackId;

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
        // MVP: tracks table with stable id + unique path.
        // Expand later (mtime/size/fingerprint, cached tags, etc).
        self.conn
            .execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA foreign_keys = ON;

                CREATE TABLE IF NOT EXISTS tracks (
                    id   INTEGER PRIMARY KEY,
                    path TEXT NOT NULL UNIQUE
                );
                "#,
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Ensure each path exists as a row and return TrackIds in the same order.
    ///
    /// Implementation detail:
    /// - INSERT OR IGNORE to create row
    /// - SELECT id to fetch the stable primary key
    pub fn upsert_paths(&mut self, paths: &[PathBuf]) -> Result<Vec<(TrackId, PathBuf)>, String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;

        let mut out: Vec<(TrackId, PathBuf)> = Vec::with_capacity(paths.len());

        {
            let mut insert = tx
                .prepare("INSERT OR IGNORE INTO tracks(path) VALUES (?1)")
                .map_err(|e| e.to_string())?;

            let mut select = tx
                .prepare("SELECT id FROM tracks WHERE path = ?1")
                .map_err(|e| e.to_string())?;

            for p in paths {
                let p_str = p.to_string_lossy();

                insert
                    .execute(params![p_str.as_ref()])
                    .map_err(|e| e.to_string())?;

                let id_i64: i64 = select
                    .query_row(params![p_str.as_ref()], |row| row.get(0))
                    .map_err(|e| e.to_string())?;

                out.push((TrackId(id_i64), p.clone()));
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(out)
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
