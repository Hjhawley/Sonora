//! Core data types shared between core logic and the UI.
//!
//! Rule of thumb:
//! - These structs should be “boring bags of data”
//! - No GUI code
//! - No filesystem code
//! - No tag parsing code
//!
//! Why?
//! - Easy to display in UI
//! - Easy to serialize later (JSON/DB)
//! - Easy to unit test
//!
//! 'TrackRow' represents ONE audio file on disk plus the metadata we care about.

use std::path::PathBuf;

/// Minimal "row" of track metadata for display.
/// Think: one line in a table or list.
///
/// Rust newbie translation:
/// - 'Option<T>' means “maybe a value”
///   - Some(value) = we have it
///   - None = missing/unknown
///
/// We use Option for metadata because:
/// - some files have missing tags
/// - some tags fail to read
/// - we want the UI to handle that gracefully (“Unknown Artist”) instead of crashing
#[derive(Debug, Clone)]
pub struct TrackRow {
    /// Full file path on disk.
    /// This is the only thing we always have.
    pub path: PathBuf,

    /// ID3 Title (Song name)
    pub title: Option<String>,

    /// ID3 Artist (Per-track artist)
    pub artist: Option<String>,

    /// ID3 Album (Album name)
    pub album: Option<String>,

    /// Track number (like 1, 2, 3...)
    pub track_no: Option<u32>,

    /// Release year (like 1998)
    pub year: Option<i32>,
}
