//! core/hydrate.rs
//!
//! Metadata hydration from disk.
//!
//! Turns DB-backed `(TrackId, PathBuf)` pairs into fully populated TrackRows
//! by reading tags/probed metadata from the source files.

use std::path::PathBuf;

use crate::core::tags;
use crate::core::types::{TrackId, TrackRow};

/// Hydrate real on-disk metadata for a DB-backed `(TrackId, PathBuf)` list.
///
/// Use this during scan/save refreshes, not at startup.
pub fn hydrate_tracks(id_paths: Vec<(TrackId, PathBuf)>) -> (Vec<TrackRow>, usize) {
    let mut rows: Vec<TrackRow> = Vec::with_capacity(id_paths.len());
    let mut tag_failures: usize = 0;

    for (id, path) in id_paths {
        let (mut row, failed) = tags::read_track_row(path);
        if failed {
            tag_failures += 1;
        }
        row.id = Some(id);
        rows.push(row);
    }

    (rows, tag_failures)
}
