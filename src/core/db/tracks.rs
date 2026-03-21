//! core/db/tracks.rs
//!
//! Track-record persistence, loading, and Sonora-only DB actions.

use std::path::PathBuf;

use rusqlite::{Row, params};

use super::Db;
use crate::core::types::{TrackId, TrackRow};

impl Db {
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

    pub fn load_visible_tracks(&self) -> Result<Vec<TrackRow>, String> {
        self.load_tracks_where("present = 1 AND hidden = 0")
    }

    pub fn load_hidden_tracks(&self) -> Result<Vec<TrackRow>, String> {
        self.load_tracks_where("present = 1 AND hidden = 1")
    }

    pub fn load_missing_tracks(&self) -> Result<Vec<TrackRow>, String> {
        self.load_tracks_where("present = 0")
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
