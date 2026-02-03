//! ID3 tag reading utilities.
//!
//! We use the `id3` crate to read tags from MP3 files.
//!
//! Important philosophy:
//! - Tag read failures should be survivable.
//! - A missing/corrupt tag should NOT cancel the whole scan.
//!
//! So `read_track_row` returns `(TrackRow, bool)` where bool = "failed to read tags".

use std::path::PathBuf;

use id3::{Tag, TagLike};

use super::types::TrackRow;

/// Read metadata from a single MP3 file and convert it into a `TrackRow`.
///
/// Returns:
/// - `(TrackRow, false)` if tags were read successfully
/// - `(TrackRow, true)` if tag reading failed (TrackRow will have None metadata)
pub fn read_track_row(path: PathBuf) -> (TrackRow, bool) {
    match Tag::read_from_path(&path) {
        Ok(tag) => (
            TrackRow {
                path,
                title: tag.title().map(str::to_owned),
                artist: tag.artist().map(str::to_owned),
                album: tag.album().map(str::to_owned),
                track_no: tag.track(),
                year: tag.year(),
            },
            false,
        ),
        Err(_) => (
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
