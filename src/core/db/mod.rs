//! core/db/mod.rs
//!
//! SQLite persistence boundary for Sonora.
//!
//! This module owns:
//! - the Connection-backed 'Db' type
//! - database opening and schema initialization
//! - submodule wiring for roots, reconciliation, and track persistence
//! - small persisted application-state values
//! - the conversion policy for filesystem paths stored as SQLite TEXT

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

mod paths;
mod reconcile;
mod roots;
mod schema;
mod tracks;

pub use paths::default_db_path;

const APP_STATE_VOLUME_KEY: &str = "volume";
const APP_STATE_VIEW_MODE_KEY: &str = "view_mode";

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(db_path: &Path) -> Result<Self, String> {
        let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
        let db = Self { conn };

        db.init_schema()?;

        Ok(db)
    }

    pub fn load_volume(&self) -> Result<Option<f32>, String> {
        let Some(raw) = self.load_app_state(APP_STATE_VOLUME_KEY)? else {
            return Ok(None);
        };

        let volume = raw
            .parse::<f32>()
            .map_err(|e| format!("Invalid persisted volume value {raw:?}: {e}"))?;

        Ok(Some(volume.clamp(0.0, 1.0)))
    }

    pub fn save_volume(&self, volume: f32) -> Result<(), String> {
        self.save_app_state(APP_STATE_VOLUME_KEY, &volume.clamp(0.0, 1.0).to_string())
    }

    pub fn load_view_mode(&self) -> Result<Option<String>, String> {
        self.load_app_state(APP_STATE_VIEW_MODE_KEY)
    }

    pub fn save_view_mode(&self, view_mode: &str) -> Result<(), String> {
        self.save_app_state(APP_STATE_VIEW_MODE_KEY, view_mode)
    }

    fn load_app_state(&self, key: &str) -> Result<Option<String>, String> {
        self.conn
            .query_row(
                "SELECT value FROM app_state WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())
    }

    fn save_app_state(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn
            .execute(
                r#"
                INSERT INTO app_state(key, value)
                VALUES (?1, ?2)
                ON CONFLICT(key) DO UPDATE SET value = excluded.value
                "#,
                params![key, value],
            )
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}

/// Convert a filesystem path into Sonora's SQLite TEXT representation.
/// Reject unsupported paths.
pub(super) fn path_to_db_text(path: &Path) -> Result<&str, String> {
    match path.to_str() {
        Some(path_text) => Ok(path_text),
        None => Err(format!(
            "Sonora cannot store a non-UTF-8 filesystem path: {}",
            path.display()
        )),
    }
}
