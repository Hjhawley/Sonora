//! core/db/tracks.rs
//!
//! Track-row persistence, DB-backed loading, SQL-to-TrackRow mapping, and
//! Sonora-only row actions.

use std::path::PathBuf;

use rusqlite::{Error as SqlError, Row, params, types::Type};

use super::Db;
use super::schema::TRACK_METADATA_CACHE_VERSION;
use crate::core::types::{TrackId, TrackRow};

impl Db {
    /// Update metadata for rows that already exist in the track cache.
    ///
    /// Reconciliation creates the underlying rows and assigns their TrackIds.
    /// A successful metadata update stamps each row with the current cache
    /// version so future scans can skip unchanged files.
    pub fn update_track_rows_metadata(&mut self, rows: &[TrackRow]) -> Result<(), String> {
        if rows.is_empty() {
            return Ok(());
        }

        let transaction = self.conn.transaction().map_err(|error| error.to_string())?;

        {
            let mut update = transaction
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

                        track_no_text = ?11,
                        track_total_text = ?12,
                        disc_no_text = ?13,
                        disc_total_text = ?14,

                        release_date = ?15,
                        year = ?16,
                        genre = ?17,

                        grouping = ?18,
                        content_group = ?19,
                        comment_text = ?20,
                        lyrics = ?21,
                        lyricist = ?22,
                        conductor = ?23,
                        remixer = ?24,
                        publisher = ?25,
                        subtitle = ?26,
                        bpm = ?27,
                        key_text = ?28,
                        mood = ?29,
                        language = ?30,
                        isrc = ?31,
                        encoder_settings = ?32,
                        encoded_by = ?33,
                        copyright = ?34,

                        artwork_count = ?35,
                        title_sort = ?36,
                        artist_sort = ?37,
                        album_sort = ?38,
                        album_artist_sort = ?39,

                        duration_ms = ?40,
                        bitrate_kbps = ?41,
                        sample_rate_hz = ?42,
                        channels = ?43,

                        rating = ?44,
                        play_count = ?45,
                        compilation = ?46,
                        metadata_cache_version = ?47
                    WHERE id = ?1
                    "#,
                )
                .map_err(|error| error.to_string())?;

            for row in rows {
                let id = match row.id {
                    Some(id) => id,
                    None => {
                        return Err(format!(
                            "Cannot cache metadata for {} without a TrackId",
                            row.path.display()
                        ));
                    }
                };

                let play_count = row
                    .play_count
                    .map(|value| {
                        i64::try_from(value).map_err(|_| {
                            format!(
                                "Play count does not fit SQLite INTEGER for {}",
                                row.path.display()
                            )
                        })
                    })
                    .transpose()?;

                let affected = update
                    .execute(params![
                        id.0,
                        row.title.as_deref(),
                        row.artist.as_deref(),
                        row.album.as_deref(),
                        row.album_artist.as_deref(),
                        row.composer.as_deref(),
                        row.track_no.map(i64::from),
                        row.track_total.map(i64::from),
                        row.disc_no.map(i64::from),
                        row.disc_total.map(i64::from),
                        row.track_no_text.as_deref(),
                        row.track_total_text.as_deref(),
                        row.disc_no_text.as_deref(),
                        row.disc_total_text.as_deref(),
                        row.release_date.as_deref(),
                        row.year,
                        row.genre.as_deref(),
                        row.grouping.as_deref(),
                        row.content_group.as_deref(),
                        row.comment.as_deref(),
                        row.lyrics.as_deref(),
                        row.lyricist.as_deref(),
                        row.conductor.as_deref(),
                        row.remixer.as_deref(),
                        row.publisher.as_deref(),
                        row.subtitle.as_deref(),
                        row.bpm.map(i64::from),
                        row.key.as_deref(),
                        row.mood.as_deref(),
                        row.language.as_deref(),
                        row.isrc.as_deref(),
                        row.encoder_settings.as_deref(),
                        row.encoded_by.as_deref(),
                        row.copyright.as_deref(),
                        i64::from(row.artwork_count),
                        row.title_sort.as_deref(),
                        row.artist_sort.as_deref(),
                        row.album_sort.as_deref(),
                        row.album_artist_sort.as_deref(),
                        row.duration_ms.map(i64::from),
                        row.bitrate_kbps.map(i64::from),
                        row.sample_rate_hz.map(i64::from),
                        row.channels.map(i64::from),
                        row.rating.map(i64::from),
                        play_count,
                        row.compilation
                            .map(|value| if value { 1_i64 } else { 0_i64 }),
                        TRACK_METADATA_CACHE_VERSION,
                    ])
                    .map_err(|error| error.to_string())?;

                if affected != 1 {
                    return Err(format!(
                        "Expected to update track {}, but updated {affected} rows",
                        id.0
                    ));
                }
            }
        }

        transaction.commit().map_err(|error| error.to_string())?;

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

                track_no_text,
                track_total_text,
                disc_no_text,
                disc_total_text,

                release_date,
                year,
                genre,

                grouping,
                content_group,
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

        let mut statement = self.conn.prepare(&sql).map_err(|error| error.to_string())?;

        let rows = statement
            .query_map([], Self::track_row_from_sql_row)
            .map_err(|error| error.to_string())?;

        let mut tracks = Vec::new();

        for row in rows {
            tracks.push(row.map_err(|error| error.to_string())?);
        }

        Ok(tracks)
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

            track_no: optional_u32(row, 7)?,
            track_total: optional_u32(row, 8)?,
            disc_no: optional_u32(row, 9)?,
            disc_total: optional_u32(row, 10)?,

            track_no_text: row.get(11)?,
            track_total_text: row.get(12)?,
            disc_no_text: row.get(13)?,
            disc_total_text: row.get(14)?,

            release_date: row.get(15)?,
            year: row.get(16)?,
            genre: row.get(17)?,

            grouping: row.get(18)?,
            content_group: row.get(19)?,
            comment: row.get(20)?,
            lyrics: row.get(21)?,
            lyricist: row.get(22)?,
            conductor: row.get(23)?,
            remixer: row.get(24)?,
            publisher: row.get(25)?,
            subtitle: row.get(26)?,
            bpm: optional_u32(row, 27)?,
            key: row.get(28)?,
            mood: row.get(29)?,
            language: row.get(30)?,
            isrc: row.get(31)?,
            encoder_settings: row.get(32)?,
            encoded_by: row.get(33)?,
            copyright: row.get(34)?,

            artwork_count: required_u32(row, 35)?,
            title_sort: row.get(36)?,
            artist_sort: row.get(37)?,
            album_sort: row.get(38)?,
            album_artist_sort: row.get(39)?,

            duration_ms: optional_u32(row, 40)?,
            bitrate_kbps: optional_u32(row, 41)?,
            sample_rate_hz: optional_u32(row, 42)?,
            channels: optional_u8(row, 43)?,

            rating: optional_u8(row, 44)?,
            play_count: optional_u64(row, 45)?,
            compilation: row.get::<_, Option<i64>>(46)?.map(|value| value != 0),
        })
    }

    pub fn set_hidden(&self, id: TrackId, hidden: bool) -> Result<(), String> {
        let affected = self
            .conn
            .execute(
                "UPDATE tracks SET hidden = ?2 WHERE id = ?1",
                params![id.0, if hidden { 1_i64 } else { 0_i64 }],
            )
            .map_err(|error| error.to_string())?;

        if affected != 1 {
            return Err(format!(
                "Track {} does not exist in the Sonora database",
                id.0
            ));
        }

        Ok(())
    }

    pub fn delete_track(&self, id: TrackId) -> Result<(), String> {
        let affected = self
            .conn
            .execute("DELETE FROM tracks WHERE id = ?1", params![id.0])
            .map_err(|error| error.to_string())?;

        if affected != 1 {
            return Err(format!(
                "Track {} does not exist in the Sonora database",
                id.0
            ));
        }

        Ok(())
    }
}

fn required_u32(row: &Row<'_>, index: usize) -> rusqlite::Result<u32> {
    let value = row.get::<_, i64>(index)?;
    convert_integer(index, value)
}

fn optional_u32(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u32>> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| convert_integer(index, value))
        .transpose()
}

fn optional_u8(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u8>> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| convert_integer(index, value))
        .transpose()
}

fn optional_u64(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)?
        .map(|value| convert_integer(index, value))
        .transpose()
}

fn convert_integer<T>(index: usize, value: i64) -> rusqlite::Result<T>
where
    T: TryFrom<i64>,
    T::Error: std::error::Error + Send + Sync + 'static,
{
    T::try_from(value)
        .map_err(|error| SqlError::FromSqlConversionFailure(index, Type::Integer, Box::new(error)))
}
