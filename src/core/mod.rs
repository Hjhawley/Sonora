//! core/mod.rs
//!
//! The brain of the app:
//! - Walk the filesystem for audio files
//! - Read/write tags
//! - Return plain data structs for the GUI to render

pub mod library;
pub mod playback;
pub mod tags;
pub mod types;

use std::collections::HashSet;
use std::path::PathBuf;

use types::TrackRow;

/// Scan one or more folder roots for `.mp3` files, then read ID3 tags for each file.
///
/// Behavior:
/// - Filesystem scan errors are fatal and return `Err(String)` (e.g., root doesn't exist).
/// - Tag read errors are non-fatal:
///   - the track is still returned with empty/`None` metadata
///   - the returned `usize` counts how many tag reads failed
///
/// Dedupe:
/// - If roots overlap, the same file path may appear multiple times.
/// - We de-duplicate by full `PathBuf` so each file is processed at most once.
///
/// Ordering:
/// - Returned rows are sorted by full path
pub fn scan_and_read_roots(roots: &[PathBuf]) -> Result<(Vec<TrackRow>, usize), String> {
    let mut seen: HashSet<PathBuf> = HashSet::with_capacity(1024);
    let mut all_paths: Vec<PathBuf> = Vec::new();

    // Gather unique paths
    for root in roots {
        let paths = library::scan_mp3s(root)?;
        for path in paths {
            if seen.insert(path.clone()) {
                all_paths.push(path);
            }
        }
    }

    // Sort once, in core (GUI should never sort)
    all_paths.sort();

    // Read tags in that order
    let mut rows: Vec<TrackRow> = Vec::with_capacity(all_paths.len());
    let mut tag_failures: usize = 0;

    for path in all_paths {
        let (row, failed) = tags::read_track_row(path);
        if failed {
            tag_failures += 1;
        }
        rows.push(row);
    }

    Ok((rows, tag_failures))
}
