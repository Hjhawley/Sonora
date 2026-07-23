//! core/types.rs
//!
//! Canonical application-level domain types shared across filesystem
//! discovery, metadata tags, SQLite caching, scan/load flows, GUI state, and
//! query derivation.
//!
//! This module contains data definitions only. It does not perform GUI work,
//! database access, tag parsing, probing, or playback.

use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrackId(pub i64);

impl fmt::Display for TrackId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone)]
pub struct TrackRow {
    pub id: Option<TrackId>,
    pub path: PathBuf,

    // Editable textual metadata.
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub composer: Option<String>,

    // Editable track and disc counters.
    pub track_no: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_no: Option<u32>,
    pub disc_total: Option<u32>,

    // Editable release metadata.
    pub release_date: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,

    // Additional editable metadata.
    pub grouping: Option<String>,
    pub comment: Option<String>,
    pub lyrics: Option<String>,
    pub lyricist: Option<String>,
    pub conductor: Option<String>,
    pub remixer: Option<String>,
    pub publisher: Option<String>,
    pub subtitle: Option<String>,
    pub bpm: Option<u32>,
    pub key: Option<String>,
    pub mood: Option<String>,
    pub language: Option<String>,
    pub isrc: Option<String>,
    pub encoder_settings: Option<String>,
    pub encoded_by: Option<String>,
    pub copyright: Option<String>,

    // Artwork and sorting helpers.
    pub artwork_count: u32,
    pub title_sort: Option<String>,
    pub artist_sort: Option<String>,
    pub album_sort: Option<String>,
    pub album_artist_sort: Option<String>,

    // Read-only technical media properties.
    pub duration_ms: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,

    // Additional tag-derived library metadata.
    pub rating: Option<u8>,
    pub play_count: Option<u64>,
    pub compilation: Option<bool>,
}

impl TrackRow {
    /// Construct an empty row associated with a filesystem path.
    ///
    /// Tag readers and probes populate fields after construction. A TrackId is
    /// assigned separately when the row is associated with a SQLite record.
    pub fn empty(path: PathBuf) -> Self {
        Self {
            id: None,
            path,

            title: None,
            artist: None,
            album: None,
            album_artist: None,
            composer: None,

            track_no: None,
            track_total: None,
            disc_no: None,
            disc_total: None,

            release_date: None,
            year: None,
            genre: None,

            grouping: None,
            comment: None,
            lyrics: None,
            lyricist: None,
            conductor: None,
            remixer: None,
            publisher: None,
            subtitle: None,
            bpm: None,
            key: None,
            mood: None,
            language: None,
            isrc: None,
            encoder_settings: None,
            encoded_by: None,
            copyright: None,

            artwork_count: 0,
            title_sort: None,
            artist_sort: None,
            album_sort: None,
            album_artist_sort: None,

            duration_ms: None,
            bitrate_kbps: None,
            sample_rate_hz: None,
            channels: None,

            rating: None,
            play_count: None,
            compilation: None,
        }
    }
}
