// core/types.rs
//
// Core, UI-agnostic domain types.
//
// **Rule of thumb:** this module is *data*, not behavior.
// - No GUI types
// - No filesystem walking
// - No ID3 / tag parsing
// - No database code
//
// `TrackRow` is the app’s canonical “one file + metadata we care about” record.
// The `core::tags` layer *produces* and *consumes* it, and the GUI renders/edits it.
//
// We use lots of `Option<T>` because real libraries are messy:
// - tags missing or empty
// - unreadable / corrupt tags
// - inconsistent tagging conventions
//
// Conventions (important):
// - **`None`** means “absent / unknown / unreadable”
// - **`Some(s)`** should be non-empty and trimmed (normalized for UI sanity)
//   (i.e., treat empty/whitespace as `None` during read + inspector parsing)

use std::collections::BTreeMap;
use std::path::PathBuf;

// Stable identifier for a track.
//
// Why have this?
// - `Vec` indices are not stable (rescans, sorts, inserts, deletes)
// - once SQLite exists, this becomes the DB primary key
//
// MVP note:
// - while you don't have a DB yet, `TrackRow::id` may be `None`
// - once SQLite lands, every row should have `Some(id)`
//
// We choose `i64` because it matches SQLite `INTEGER PRIMARY KEY` nicely.
pub type TrackId = i64;

// Minimal "row" of track metadata for display/edit.
// One `TrackRow` = one audio file + the metadata we know about it.
//
// This struct is intentionally **format-agnostic**: it describes *music metadata*,
// not "ID3 tags". The tags layer is responsible for mapping between containers
// (MP3/ID3 today) and this record.
#[derive(Debug, Clone)]
pub struct TrackRow {
    // Stable identity (DB primary key once SQLite is added).
    //
    // - `None` in the pre-DB MVP can be acceptable.
    // - Once SQLite is introduced, treat `None` as a bug.
    pub id: Option<TrackId>,

    // Canonical file location for this track.
    //
    // MVP identity is still effectively path-based; later the DB owns identity and
    // this becomes “where it currently lives”.
    pub path: PathBuf,

    // Core display/edit tags (shown by default)
    // Song title (ID3: `TIT2`)
    pub title: Option<String>,

    // Track artist (ID3: `TPE1`)
    pub artist: Option<String>,

    // Album title (ID3: `TALB`)
    pub album: Option<String>,

    // Album artist (ID3: `TPE2`)
    //
    // This is *the* field you want for Album View grouping/sorting.
    pub album_artist: Option<String>,

    // Composer (ID3: `TCOM`)
    pub composer: Option<String>,

    // Track number (ID3: `TRCK`, "1/12" -> 1)
    pub track_no: Option<u32>,

    // Total tracks on the album (ID3: `TRCK`, "1/12" -> 12)
    pub track_total: Option<u32>,

    // Disc number (ID3: `TPOS`, "1/2" -> 1)
    pub disc_no: Option<u32>,

    // Total discs (ID3: `TPOS`, "1/2" -> 2)
    pub disc_total: Option<u32>,

    // Year (best effort) (ID3: `TYER` / `TDRC`)
    pub year: Option<i32>,

    // Full date string when available (often "YYYY-MM-DD") (ID3: `TDRC`).
    //
    // Stored as a `String` because formats vary wildly in the wild.
    pub date: Option<String>,

    // Genre (ID3: `TCON`)
    pub genre: Option<String>,

    // Common extended tags (usually behind a UI toggle)
    // Grouping / content group (ID3: `TIT1`)
    pub grouping: Option<String>,

    // A short comment (ID3: `COMM`).
    // If multiple comment frames exist, keep the first one.
    pub comment: Option<String>,

    // Unsynced lyrics (ID3: `USLT`).
    // If multiple lyrics frames exist, keep the first one.
    pub lyrics: Option<String>,

    // Lyricist / text writer (ID3: `TEXT`)
    pub lyricist: Option<String>,

    // Conductor (ID3: `TPE3`)
    pub conductor: Option<String>,

    // Remixer / modifier (ID3: `TPE4`)
    pub remixer: Option<String>,

    // Publisher / label (ID3: `TPUB`)
    pub publisher: Option<String>,

    // Subtitle / refinement (ID3: `TIT3`)
    pub subtitle: Option<String>,

    // BPM (beats per minute) (ID3: `TBPM`)
    pub bpm: Option<u32>,

    // Musical key (ID3: `TKEY`)
    pub key: Option<String>,

    // Mood (ID3: `TMOO`)
    pub mood: Option<String>,

    // Language(s) (ID3: `TLAN`)
    pub language: Option<String>,

    // ISRC (recording code) (ID3: `TSRC`)
    pub isrc: Option<String>,

    // Encoder settings / software profile (ID3: `TSSE`)
    pub encoder_settings: Option<String>,

    // Encoded-by (human/organization) (ID3: `TENC`)
    pub encoded_by: Option<String>,

    // Copyright (ID3: `TCOP`)
    pub copyright: Option<String>,

    // Artwork + sort fields (important for UX correctness)
    // Embedded album artwork count (ID3: `APIC` / `PIC` frames).
    //
    // We store **count only** to keep `TrackRow` lightweight. Actual image bytes
    // are fetched on-demand (e.g. cover thumbnails).
    pub artwork_count: u32,

    // Title sort (ID3: `TSOT`)
    pub title_sort: Option<String>,

    // Artist sort (ID3: `TSOP`)
    pub artist_sort: Option<String>,

    // Album sort (ID3: `TSOA`)
    pub album_sort: Option<String>,

    // Album artist sort (ID3: `TSO2`)
    pub album_artist_sort: Option<String>,

    // Library/stats-ish fields (not always present, but useful)
    // Duration in milliseconds (ID3: `TLEN`).
    //
    // Many files/libraries do not store this reliably; treat as optional hint.
    pub duration_ms: Option<u32>,

    // Rating (0–255 in `POPM`; stored as raw byte).
    pub rating: Option<u8>,

    // Play count (`POPM` counter or `PCNT`).
    pub play_count: Option<u64>,

    // Compilation flag (commonly `TCMP` or `TXXX:COMPILATION`).
    pub compilation: Option<bool>,

    // Escape hatches: preserve unknown/extra tags without redesigning the struct
    // User-defined text frames (ID3: `TXXX`).
    // Key = description, Value = value.
    pub user_text: BTreeMap<String, String>,

    // URL frames.
    // Key = frame id or description (for `WXXX`), Value = URL.
    pub urls: BTreeMap<String, String>,

    // Any other text-ish frames we didn't explicitly model.
    // Key = frame id (e.g. "TOPE"), Value = best-effort text value.
    pub extra_text: BTreeMap<String, String>,
}
