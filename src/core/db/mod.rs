//! core/db/mod.rs
//!
//! SQLite persistence layer.
//! - DB owns stable identity
//! - DB stores enough metadata for fast startup / scope loads
//! - scan only hydrates new/changed files
//! - old rows are backfilled once if cached metadata is still empty

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::core::library::DiscoveredFile;
use crate::core::types::{TrackId, TrackRow};

mod paths;
mod schema;

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
}

impl Db {
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

            if !path.starts_with(removed_root) {
                continue;
            }

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
    /// Reconcile filesystem facts into DB truth for the scanned roots.
    ///
    /// Returns DB-backed `(TrackId, PathBuf)` items whose metadata should be
    /// re-read from disk because the file is:
    /// - new
    /// - changed by `(mtime, size)`
    /// - or still missing cached metadata in the DB
    pub fn upsert_discovered(
        &mut self,
        scanned_roots: &[PathBuf],
        files: &[DiscoveredFile],
    ) -> Result<Vec<(TrackId, PathBuf)>, String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;

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

        let mut changed: Vec<(TrackId, PathBuf)> = Vec::new();

        {
            let mut select_existing = tx
                .prepare(
                    r#"
                    SELECT
                        id,
                        mtime,
                        size,
                        title,
                        artist,
                        album,
                        album_artist,
                        duration_ms
                    FROM tracks
                    WHERE path = ?1
                    "#,
                )
                .map_err(|e| e.to_string())?;

            let mut insert = tx
                .prepare("INSERT OR IGNORE INTO tracks(path) VALUES (?1)")
                .map_err(|e| e.to_string())?;

            let mut update_presence = tx
                .prepare(
                    r#"
                    UPDATE tracks
                    SET present = 1, mtime = ?2, size = ?3
                    WHERE path = ?1
                    "#,
                )
                .map_err(|e| e.to_string())?;

            let mut select_id = tx
                .prepare("SELECT id FROM tracks WHERE path = ?1")
                .map_err(|e| e.to_string())?;

            for f in files {
                let p_str = f.path.to_string_lossy();
                let new_size_i64 = f.size.map(|s| s as i64);

                let existing: Option<(
                    TrackId,
                    Option<i64>,
                    Option<i64>,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<i64>,
                )> = select_existing
                    .query_row(params![p_str.as_ref()], |row| {
                        Ok((
                            TrackId(row.get::<_, i64>(0)?),
                            row.get::<_, Option<i64>>(1)?,
                            row.get::<_, Option<i64>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, Option<String>>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, Option<String>>(6)?,
                            row.get::<_, Option<i64>>(7)?,
                        ))
                    })
                    .optional()
                    .map_err(|e| e.to_string())?;

                let needs_refresh = match existing {
                    None => true,
                    Some((
                        _id,
                        old_mtime,
                        old_size,
                        title,
                        artist,
                        album,
                        album_artist,
                        duration_ms,
                    )) => {
                        let metadata_missing = title.is_none()
                            && artist.is_none()
                            && album.is_none()
                            && album_artist.is_none()
                            && duration_ms.is_none();

                        old_mtime != f.mtime_unix || old_size != new_size_i64 || metadata_missing
                    }
                };

                insert
                    .execute(params![p_str.as_ref()])
                    .map_err(|e| e.to_string())?;

                update_presence
                    .execute(params![p_str.as_ref(), f.mtime_unix, new_size_i64])
                    .map_err(|e| e.to_string())?;

                if needs_refresh {
                    let id = TrackId(
                        select_id
                            .query_row(params![p_str.as_ref()], |row| row.get::<_, i64>(0))
                            .map_err(|e| e.to_string())?,
                    );
                    changed.push((id, f.path.clone()));
                }
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(changed)
    }

    pub fn upsert_track_rows_metadata(&mut self, rows: &[TrackRow]) -> Result<(), String> {
        if rows.is_empty() {
            return Ok(());
        }

        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        {
            let mut update = tx
                .prepare(
                    r#"
                    UPDATE tracks
                    SET
                        title = ?2,
                        artist = ?3,
                        album = ?4,
                        album_artist = ?5,
                        composer = ?6,
                        track_no = ?7,
                        track_total = ?8,
                        disc_no = ?9,
                        disc_total = ?10,
                        release_date = ?11,
                        year = ?12,
                        genre = ?13,
                        grouping = ?14,
                        comment_text = ?15,
                        lyrics = ?16,
                        lyricist = ?17,
                        conductor = ?18,
                        remixer = ?19,
                        publisher = ?20,
                        subtitle = ?21,
                        bpm = ?22,
                        key_text = ?23,
                        mood = ?24,
                        language = ?25,
                        isrc = ?26,
                        encoder_settings = ?27,
                        encoded_by = ?28,
                        copyright = ?29,
                        artwork_count = ?30,
                        title_sort = ?31,
                        artist_sort = ?32,
                        album_sort = ?33,
                        album_artist_sort = ?34,
                        duration_ms = ?35,
                        bitrate_kbps = ?36,
                        sample_rate_hz = ?37,
                        channels = ?38,
                        rating = ?39,
                        play_count = ?40,
                        compilation = ?41
                    WHERE id = ?1
                    "#,
                )
                .map_err(|e| e.to_string())?;

            for row in rows {
                let Some(id) = row.id else {
                    continue;
                };

                update
                    .execute(params![
                        id.0,
                        row.title.as_deref(),
                        row.artist.as_deref(),
                        row.album.as_deref(),
                        row.album_artist.as_deref(),
                        row.composer.as_deref(),
                        row.track_no.map(|v| v as i64),
                        row.track_total.map(|v| v as i64),
                        row.disc_no.map(|v| v as i64),
                        row.disc_total.map(|v| v as i64),
                        row.release_date.as_deref(),
                        row.year,
                        row.genre.as_deref(),
                        row.grouping.as_deref(),
                        row.comment.as_deref(),
                        row.lyrics.as_deref(),
                        row.lyricist.as_deref(),
                        row.conductor.as_deref(),
                        row.remixer.as_deref(),
                        row.publisher.as_deref(),
                        row.subtitle.as_deref(),
                        row.bpm.map(|v| v as i64),
                        row.key.as_deref(),
                        row.mood.as_deref(),
                        row.language.as_deref(),
                        row.isrc.as_deref(),
                        row.encoder_settings.as_deref(),
                        row.encoded_by.as_deref(),
                        row.copyright.as_deref(),
                        row.artwork_count as i64,
                        row.title_sort.as_deref(),
                        row.artist_sort.as_deref(),
                        row.album_sort.as_deref(),
                        row.album_artist_sort.as_deref(),
                        row.duration_ms.map(|v| v as i64),
                        row.bitrate_kbps.map(|v| v as i64),
                        row.sample_rate_hz.map(|v| v as i64),
                        row.channels.map(|v| v as i64),
                        row.rating.map(|v| v as i64),
                        row.play_count.map(|v| v as i64),
                        row.compilation.map(|v| if v { 1_i64 } else { 0_i64 }),
                    ])
                    .map_err(|e| e.to_string())?;
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn load_visible_paths(&self) -> Result<Vec<(TrackId, PathBuf)>, String> {
        self.load_id_paths_where("present = 1 AND hidden = 0")
    }

    pub fn load_hidden_paths(&self) -> Result<Vec<(TrackId, PathBuf)>, String> {
        self.load_id_paths_where("present = 1 AND hidden = 1")
    }

    pub fn load_missing_paths(&self) -> Result<Vec<(TrackId, PathBuf)>, String> {
        self.load_id_paths_where("present = 0")
    }

    pub fn load_visible_tracks(&self) -> Result<Vec<TrackRow>, String> {
        self.load_tracks_where("present = 1 AND hidden = 0")
    }

    pub fn load_hidden_tracks(&self) -> Result<Vec<TrackRow>, String> {
        self.load_tracks_where("present = 1 AND hidden = 1")
    }

    pub fn load_missing_tracks(&self) -> Result<Vec<TrackRow>, String> {
        self.load_tracks_where("present = 0")
    }

    fn load_id_paths_where(&self, where_sql: &str) -> Result<Vec<(TrackId, PathBuf)>, String> {
        let sql = format!(
            r#"
            SELECT id, path
            FROM tracks
            WHERE {where_sql}
            ORDER BY path
            "#
        );

        let mut stmt = self.conn.prepare(&sql).map_err(|e| e.to_string())?;
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

    fn load_tracks_where(&self, where_sql: &str) -> Result<Vec<TrackRow>, String> {
        let sql = format!(
            r#"
            SELECT
                id,
                path,
                title,
                artist,
                album,
                album_artist,
                composer,
                track_no,
                track_total,
                disc_no,
                disc_total,
                release_date,
                year,
                genre,
                grouping,
                comment_text,
                lyrics,
                lyricist,
                conductor,
                remixer,
                publisher,
                subtitle,
                bpm,
                key_text,
                mood,
                language,
                isrc,
                encoder_settings,
                encoded_by,
                copyright,
                artwork_count,
                title_sort,
                artist_sort,
                album_sort,
                album_artist_sort,
                duration_ms,
                bitrate_kbps,
                sample_rate_hz,
                channels,
                rating,
                play_count,
                compilation
            FROM tracks
            WHERE {where_sql}
            ORDER BY path
            "#
        );

        let mut stmt = self.conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], Self::track_row_from_sql_row)
            .map_err(|e| e.to_string())?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    fn track_row_from_sql_row(row: &Row<'_>) -> rusqlite::Result<TrackRow> {
        let id = TrackId(row.get::<_, i64>(0)?);
        let path = PathBuf::from(row.get::<_, String>(1)?);

        Ok(TrackRow {
            id: Some(id),
            path,

            title: row.get(2)?,
            artist: row.get(3)?,
            album: row.get(4)?,
            album_artist: row.get(5)?,
            composer: row.get(6)?,

            track_no: row.get::<_, Option<i64>>(7)?.map(|v| v as u32),
            track_total: row.get::<_, Option<i64>>(8)?.map(|v| v as u32),
            disc_no: row.get::<_, Option<i64>>(9)?.map(|v| v as u32),
            disc_total: row.get::<_, Option<i64>>(10)?.map(|v| v as u32),

            release_date: row.get(11)?,
            year: row.get(12)?,
            genre: row.get(13)?,

            grouping: row.get(14)?,
            comment: row.get(15)?,
            lyrics: row.get(16)?,
            lyricist: row.get(17)?,
            conductor: row.get(18)?,
            remixer: row.get(19)?,
            publisher: row.get(20)?,
            subtitle: row.get(21)?,
            bpm: row.get::<_, Option<i64>>(22)?.map(|v| v as u32),
            key: row.get(23)?,
            mood: row.get(24)?,
            language: row.get(25)?,
            isrc: row.get(26)?,
            encoder_settings: row.get(27)?,
            encoded_by: row.get(28)?,
            copyright: row.get(29)?,

            artwork_count: row.get::<_, i64>(30)? as u32,
            title_sort: row.get(31)?,
            artist_sort: row.get(32)?,
            album_sort: row.get(33)?,
            album_artist_sort: row.get(34)?,

            duration_ms: row.get::<_, Option<i64>>(35)?.map(|v| v as u32),
            bitrate_kbps: row.get::<_, Option<i64>>(36)?.map(|v| v as u32),
            sample_rate_hz: row.get::<_, Option<i64>>(37)?.map(|v| v as u32),
            channels: row.get::<_, Option<i64>>(38)?.map(|v| v as u8),
            rating: row.get::<_, Option<i64>>(39)?.map(|v| v as u8),
            play_count: row.get::<_, Option<i64>>(40)?.map(|v| v as u64),
            compilation: row.get::<_, Option<i64>>(41)?.map(|v| v != 0),

            user_text: Default::default(),
            urls: Default::default(),
            extra_text: Default::default(),
        })
    }

    pub fn set_hidden(&self, id: TrackId, hidden: bool) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE tracks SET hidden = ?2 WHERE id = ?1",
                params![id.0, if hidden { 1 } else { 0 }],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_track(&self, id: TrackId) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM tracks WHERE id = ?1", params![id.0])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl Db {
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
