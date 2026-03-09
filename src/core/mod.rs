//! core/mod.rs
//!
//! The brain of the app:
//! - Discover candidate audio file paths (filesystem walk)
//! - Read/write tags (metadata IO)
//! - Return plain data structs for the GUI to render
//!
//! Scan pipeline boundary:
//!   (A) discover paths -> Vec<PathBuf>
//!   (B) read tags -> (Vec<TrackRow>, tag_failures)

pub mod db;
pub mod library;
pub mod playback;
pub mod tags;
pub mod types;

use std::collections::HashSet;
use std::path::PathBuf;

use types::TrackRow;

/// Discover candidate audio files under multiple roots.
///
/// - MP3-only for MVP (library enforces extension rules)
/// - De-dupes across overlapping roots by full path
/// - Sorts paths once (core owns ordering; GUI shouldn't)
pub fn scan_paths(roots: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut seen: HashSet<PathBuf> = HashSet::with_capacity(1024);
    let mut out: Vec<PathBuf> = Vec::new();

    for root in roots {
        let paths = library::scan_mp3s(root)?;
        for path in paths {
            if seen.insert(path.clone()) {
                out.push(path);
            }
        }
    }

    out.sort();
    Ok(out)
}

/// Read tags for a set of already-discovered audio paths.
///
/// - Never fails hard per-file: unreadable tags return an "empty-ish" TrackRow
/// - Returns (rows, tag_failures)
pub fn read_tracks(paths: Vec<PathBuf>) -> (Vec<TrackRow>, usize) {
    let mut rows: Vec<TrackRow> = Vec::with_capacity(paths.len());
    let mut tag_failures: usize = 0;

    for path in paths {
        let (row, failed) = tags::read_track_row(path);
        if failed {
            tag_failures += 1;
        }
        rows.push(row);
    }

    (rows, tag_failures)
}
