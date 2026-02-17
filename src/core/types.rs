//! Core data types shared between core logic and the UI.
//! - No GUI code
//! - No filesystem walking
//! - No ID3 parsing
//! This file is just plain data structs.
//! The GUI can display these, and later we can save/load them from a DB.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Minimal "row" of track metadata for display/edit.
/// One `TrackRow` = one audio file + the metadata we know about it.
///
/// `Option<T>` means "maybe a value":
/// - `Some(value)` = the tag exists
/// - `None` = missing/unknown/unreadable
///
/// We use `Option` a lot because music files are messy:
/// - missing tags
/// - corrupt tags
/// - tag read failures
/// - inconsistent tagging conventions
#[derive(Debug, Clone)]
pub struct TrackRow {
    /// 'TrackRow' represents one audio file and its metadata
    pub path: PathBuf,

    // ------------------------------------------------------------
    // "Core" tags (the ones we want visible by default in UI)
    // ------------------------------------------------------------
    /// Song title (ID3: TIT2)
    pub title: Option<String>,

    /// Track artist (ID3: TPE1)
    pub artist: Option<String>,

    /// Album title (ID3: TALB)
    pub album: Option<String>,

    /// Album artist (ID3: TPE2)
    pub album_artist: Option<String>,

    /// Composer (ID3: TCOM)
    pub composer: Option<String>,

    /// Track number (ID3: TRCK "1/12" -> 1)
    pub track_no: Option<u32>,

    /// Total tracks on the album (ID3: TRCK "1/12" -> 12)
    pub track_total: Option<u32>,

    /// Disc number (ID3: TPOS "1/2" -> 1)
    pub disc_no: Option<u32>,

    /// Total discs (ID3: TPOS "1/2" -> 2)
    pub disc_total: Option<u32>,

    /// Year (best-effort) (ID3: TYER/TDRC)
    pub year: Option<i32>,

    /// Full date string if present (often "YYYY-MM-DD") (ID3: TDRC)
    /// Kept as a String because date formats vary.
    pub date: Option<String>,

    /// Genre (ID3: TCON)
    pub genre: Option<String>,

    // ------------------------------------------------------------
    // "Common extended" tags (hidden behind toggles)
    // ------------------------------------------------------------
    /// Grouping / content group (ID3: TIT1)
    pub grouping: Option<String>,

    /// A short comment (ID3: COMM).
    /// If multiple comments exist, we keep the first one.
    pub comment: Option<String>,

    /// Unsynced lyrics (ID3: USLT).
    /// If multiple lyrics frames exist, we keep the first one.
    pub lyrics: Option<String>,

    /// Lyricist / text writer (ID3: TEXT)
    pub lyricist: Option<String>,

    /// Conductor (ID3: TPE3)
    pub conductor: Option<String>,

    /// Remixer / modifier (ID3: TPE4)
    pub remixer: Option<String>,

    /// Publisher / label (ID3: TPUB)
    pub publisher: Option<String>,

    /// Subtitle / description refinement (ID3: TIT3)
    pub subtitle: Option<String>,

    /// BPM (beats per minute) (ID3: TBPM)
    pub bpm: Option<u32>,

    /// Musical key (ID3: TKEY)
    pub key: Option<String>,

    /// Mood (ID3: TMOO)
    pub mood: Option<String>,

    /// Language(s) (ID3: TLAN)
    pub language: Option<String>,

    /// ISRC (recording code) (ID3: TSRC)
    pub isrc: Option<String>,

    /// Encoder / software used to encode (ID3: TSSE)
    pub encoder_settings: Option<String>,

    /// Encoded-by (human/organization) (ID3: TENC)
    pub encoded_by: Option<String>,

    /// Copyright (ID3: TCOP)
    pub copyright: Option<String>,

    /// Album artwork count (ID3: APIC/PIC frames).
    /// Store a count (not the bytes) to keep `TrackRow` lightweight.
    pub artwork_count: u32,

    // ------------------------------------------------------------
    // Sorting tags (nice for UI ordering / "sort by" correctness)
    // ------------------------------------------------------------
    /// Title sort (ID3: TSOT)
    pub title_sort: Option<String>,

    /// Artist sort (ID3: TSOP)
    pub artist_sort: Option<String>,

    /// Album sort (ID3: TSOA)
    pub album_sort: Option<String>,

    /// Album artist sort (ID3: TSO2)
    pub album_artist_sort: Option<String>,

    // ------------------------------------------------------------
    // "Library / stats" frames (not always present, but useful later)
    // ------------------------------------------------------------
    /// Duration in milliseconds if present (ID3: TLEN).
    /// Note: many libraries do NOT store this reliably.
    pub duration_ms: Option<u32>,

    /// Rating (0–255 in POPM; we just store raw byte).
    pub rating: Option<u8>,

    /// Play count (POPM counter or PCNT).
    pub play_count: Option<u64>,

    /// Compilation flag (varies in the wild: TCMP or TXXX:COMPILATION).
    pub compilation: Option<bool>,

    // ------------------------------------------------------------
    // "Escape hatches": keep unknown/extra tags without redesign
    // ------------------------------------------------------------
    /// User-defined text frames (ID3: TXXX).
    /// Key = description, Value = value.
    pub user_text: BTreeMap<String, String>,

    /// URL frames.
    /// Key = frame id or description (for WXXX), Value = URL.
    pub urls: BTreeMap<String, String>,

    /// Any other text-ish frames we didn't explicitly model.
    /// Key = frame id (ex: "TOPE"), Value = best-effort text value.
    pub extra_text: BTreeMap<String, String>,
}
