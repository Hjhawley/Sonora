// core/types.rs
//
// Core domain data only.
// No GUI, no DB logic, no tag parsing

use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrackId(pub i64);

impl fmt::Display for TrackId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct TrackRow {
    pub id: Option<TrackId>,
    pub path: PathBuf,

    // Editable metadata
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub composer: Option<String>,

    pub track_no: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_no: Option<u32>,
    pub disc_total: Option<u32>,

    pub release_date: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,

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

    // Artwork + sort helpers
    pub artwork_count: u32,
    pub title_sort: Option<String>,
    pub artist_sort: Option<String>,
    pub album_sort: Option<String>,
    pub album_artist_sort: Option<String>,

    // Technical read-only fields
    pub duration_ms: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
    pub rating: Option<u8>,
    pub play_count: Option<u64>,
    pub compilation: Option<bool>,
}

impl TrackRow {
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
