//! core/db/schema.rs
//!
//! Schema initialization and lightweight migration helpers.
//!
//! Current approach:
//! - Create base tables if they do not exist.
//! - Add newer columns incrementally with `ALTER TABLE ... ADD COLUMN`.
//!
//! This is intentionally simple for now. If Sonora later gains richer schema
//! evolution (playlists, smart playlists, roots, saved queues, scan runs, etc.),
//! this file is the natural place to grow explicit migration support.

use rusqlite::Connection;

use super::Db;

impl Db {
    pub(super) fn init_schema(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA foreign_keys = ON;

                CREATE TABLE IF NOT EXISTS tracks (
                    id      INTEGER PRIMARY KEY,
                    path    TEXT NOT NULL UNIQUE
                );

                CREATE TABLE IF NOT EXISTS library_roots (
                    id          INTEGER PRIMARY KEY,
                    path        TEXT NOT NULL UNIQUE,
                    enabled     INTEGER NOT NULL DEFAULT 1
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

        self.ensure_column("library_roots", "enabled", "INTEGER NOT NULL DEFAULT 1")?;

        Ok(())
    }

    pub(super) fn ensure_column(
        &self,
        table: &str,
        column: &str,
        definition: &str,
    ) -> Result<(), String> {
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
}

/// Future scaffold:
/// If schema changes become more complex than additive columns,
/// introduce versioned migrations here, likely via:
///
/// - `PRAGMA user_version`
/// - `apply_migrations(conn: &Connection)`
/// - one function per schema version step
///
/// For now, the additive-column approach is enough.
#[allow(dead_code)]
fn _future_migration_scaffold(_conn: &Connection) {}
