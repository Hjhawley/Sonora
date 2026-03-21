//! core/db/mod.rs
//! SQLite DB boundary for Sonora.
//! This module owns:
//! - the Connection-backed `Db` type
//! - database opening + schema initialization
//! - submodule wiring for roots, scan reconciliation, and track persistence
//! - small persisted app state kept in SQLite

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

mod paths;
mod reconcile;
mod roots;
mod schema;
mod tracks;

pub use paths::default_db_path;

const APP_STATE_VOLUME_KEY: &str = "volume";

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
