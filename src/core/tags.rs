//! ID3 tag reading utilities.
//!
//! This module turns an MP3 file path into a TrackRow.
//!
//! We use the 'id3' crate to read tags.
//!
//! Philosophy (important):
//! - Tag read failures are common in real libraries (missing tags, corrupt files).
//! - That should NOT crash the scan.
//! - So we return a TrackRow even when reading fails, just with 'None' metadata.
//!
//! API design:
//! - 'read_track_row(path)' returns '(TrackRow, bool)'
//!   - bool = true means "tag read failed"
//!   - bool = false means "tag read succeeded"

use std::path::PathBuf;

use id3::{Tag, TagLike};

use super::types::TrackRow;

/// Read metadata from a single MP3 file and convert it into a 'TrackRow'.
///
/// Why does it take 'PathBuf' (owned) instead of '&Path' (borrowed)?
/// - Because TrackRow stores the path.
/// - It's convenient to "move" the PathBuf into TrackRow without cloning.
///
/// Returns:
/// - '(TrackRow, false)' if tags were read successfully
/// - '(TrackRow, true)' if tag reading failed (TrackRow will have None metadata)
///
/// NOTE:
/// Right now we ignore the error details ('Err(_)').
/// Later we can store/log the error string if we want better debugging.
pub fn read_track_row(path: PathBuf) -> (TrackRow, bool) {
    match Tag::read_from_path(&path) {
        Ok(tag) => (
            TrackRow {
                path,
                // These return Option<&str>, so we map to owned Strings.
                title: tag.title().map(str::to_owned),
                artist: tag.artist().map(str::to_owned),
                album: tag.album().map(str::to_owned),

                // 'id3' crate returns Option numbers for these.
                track_no: tag.track(),
                year: tag.year(),
            },
            false,
        ),
        Err(_) => (
            // If reading failed, we still return a TrackRow with path,
            // but all metadata is None.
            TrackRow {
                path,
                title: None,
                artist: None,
                album: None,
                track_no: None,
                year: None,
            },
            true,
        ),
    }
}
