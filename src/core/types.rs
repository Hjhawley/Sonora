//! Core data types shared between core logic and the UI.
//!
//! These should be "boring structs" (plain data), so they are easy to:
//! - display in the UI
//! - save to a database later
//! - test
//!
//! `TrackRow` is intentionally simple right now. It represents one audio file.

use std::path::PathBuf;

/// Minimal "row" of track metadata for display.
/// Think: one line in a table or list.
///
/// We store `Option<String>` for fields because:
/// - some files have missing tags
/// - some files have corrupt tags
/// - we want to show "Unknown Artist" in the UI instead of crashing
#[derive(Debug, Clone)]
pub struct TrackRow {
    /// Full file path on disk.
    pub path: PathBuf,

    /// ID3 Title (or None if missing).
    pub title: Option<String>,

    /// ID3 Artist (or None if missing).
    pub artist: Option<String>,

    /// ID3 Album (or None if missing).
    pub album: Option<String>,

    /// Track number (e.g. 7).
    pub track_no: Option<u32>,

    /// Year (e.g. 2012).
    pub year: Option<i32>,
}
