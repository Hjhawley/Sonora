//! core/hydrate.rs
//!
//! Metadata hydration from disk.
//!
//! Converts DB-backed '(TrackId, PathBuf)' pairs into populated 'TrackRow'
//! values by reading tags and probing technical media properties.
//!
//! Only successful hydrations are returned for persistence. Failed files are
//! counted but omitted so their existing cached metadata is not overwritten
//! and their metadata-cache version remains stale for a later retry.

use std::path::PathBuf;

use crate::core::tags;
use crate::core::types::{TrackId, TrackRow};

/// Hydrate on-disk metadata for DB-backed track paths.
///
/// Returns:
/// - successfully hydrated rows that are safe to persist
/// - the number of files whose metadata could not be hydrated reliably
///
/// Use this during scan and save refreshes, not during normal startup.
pub fn hydrate_tracks(id_paths: Vec<(TrackId, PathBuf)>) -> (Vec<TrackRow>, usize) {
    let mut rows = Vec::with_capacity(id_paths.len());
    let mut failures = 0;

    for (id, path) in id_paths {
        let (mut row, failed) = tags::read_track_row(path);
        if failed {
            failures += 1;
            continue;
        }
        row.id = Some(id);
        rows.push(row);
    }

    (rows, failures)
}
