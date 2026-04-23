//! core/db/schema.rs
//!
//! SQLite schema + additive migrations.
//! - DB stores enough common metadata to build TrackRow directly at startup
//! - filesystem tag reads happen during scan/save, not every launch

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

        // Filesystem / visibility facts
        self.ensure_column("tracks", "present", "INTEGER NOT NULL DEFAULT 1")?;
        self.ensure_column("tracks", "hidden", "INTEGER NOT NULL DEFAULT 0")?;
        self.ensure_column("tracks", "mtime", "INTEGER")?;
        self.ensure_column("tracks", "size", "INTEGER")?;

        // Common editable/display metadata
        self.ensure_column("tracks", "title", "TEXT")?;
        self.ensure_column("tracks", "artist", "TEXT")?;
        self.ensure_column("tracks", "album", "TEXT")?;
        self.ensure_column("tracks", "album_artist", "TEXT")?;
        self.ensure_column("tracks", "composer", "TEXT")?;

        self.ensure_column("tracks", "track_no", "INTEGER")?;
        self.ensure_column("tracks", "track_total", "INTEGER")?;
        self.ensure_column("tracks", "disc_no", "INTEGER")?;
        self.ensure_column("tracks", "disc_total", "INTEGER")?;

        self.ensure_column("tracks", "release_date", "TEXT")?;
        self.ensure_column("tracks", "year", "INTEGER")?;
        self.ensure_column("tracks", "genre", "TEXT")?;

        self.ensure_column("tracks", "grouping", "TEXT")?;
        self.ensure_column("tracks", "comment_text", "TEXT")?;
        self.ensure_column("tracks", "lyrics", "TEXT")?;
        self.ensure_column("tracks", "lyricist", "TEXT")?;
        self.ensure_column("tracks", "conductor", "TEXT")?;
        self.ensure_column("tracks", "remixer", "TEXT")?;
        self.ensure_column("tracks", "publisher", "TEXT")?;
        self.ensure_column("tracks", "subtitle", "TEXT")?;
        self.ensure_column("tracks", "bpm", "INTEGER")?;
        self.ensure_column("tracks", "key_text", "TEXT")?;
        self.ensure_column("tracks", "mood", "TEXT")?;
        self.ensure_column("tracks", "language", "TEXT")?;
        self.ensure_column("tracks", "isrc", "TEXT")?;
        self.ensure_column("tracks", "encoder_settings", "TEXT")?;
        self.ensure_column("tracks", "encoded_by", "TEXT")?;
        self.ensure_column("tracks", "copyright", "TEXT")?;

        // Sort / artwork helpers
        self.ensure_column("tracks", "artwork_count", "INTEGER NOT NULL DEFAULT 0")?;
        self.ensure_column("tracks", "title_sort", "TEXT")?;
        self.ensure_column("tracks", "artist_sort", "TEXT")?;
        self.ensure_column("tracks", "album_sort", "TEXT")?;
        self.ensure_column("tracks", "album_artist_sort", "TEXT")?;

        // Read-only technical metadata
        self.ensure_column("tracks", "duration_ms", "INTEGER")?;
        self.ensure_column("tracks", "bitrate_kbps", "INTEGER")?;
        self.ensure_column("tracks", "sample_rate_hz", "INTEGER")?;
        self.ensure_column("tracks", "channels", "INTEGER")?;
        self.ensure_column("tracks", "rating", "INTEGER")?;
        self.ensure_column("tracks", "play_count", "INTEGER")?;
        self.ensure_column("tracks", "compilation", "INTEGER")?;

        // Indexes must come AFTER migrations that add the referenced columns.
        self.conn
            .execute_batch(
                r#"
                CREATE INDEX IF NOT EXISTS idx_tracks_present_hidden_path
                ON tracks(present, hidden, path);

                CREATE INDEX IF NOT EXISTS idx_tracks_present_path
                ON tracks(present, path);

                CREATE INDEX IF NOT EXISTS idx_tracks_hidden_path
                ON tracks(hidden, path);
                "#,
            )
            .map_err(|e| e.to_string())?;

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

#[allow(dead_code)]
fn _future_migration_scaffold(_conn: &Connection) {}
