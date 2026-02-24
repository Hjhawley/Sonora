//! core/mod.rs
//!
//! The brain of the app:
//! - Walk the filesystem for audio files
//! - Read/write tags
//! - Return plain data structs for the GUI to render

pub mod library;
pub mod tags;
pub mod types;
pub mod playback;

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
pub fn scan_and_read_roots(roots: &[PathBuf]) -> Result<(Vec<TrackRow>, usize), String> {
    let mut rows: Vec<TrackRow> = Vec::new();
    let mut tag_failures: usize = 0;

    // Avoid duplicates when roots overlap.
    // Note: capacity is a small optimization; total file count is unknown upfront.
    let mut seen: HashSet<PathBuf> = HashSet::with_capacity(1024);

    for root in roots {
        // 1) Find all mp3 paths under this root folder.
        let paths = library::scan_mp3s(root)?;

        // 2) Read tags for each path and convert to TrackRow.
        for path in paths {
            // insert() returns false if it was already present.
            if !seen.insert(path.clone()) {
                continue; // skip duplicates
            }

            // Tag reading never aborts the whole scan.
            let (row, failed) = tags::read_track_row(path);
            if failed {
                tag_failures += 1;
            }
            rows.push(row);
        }
    }

    Ok((rows, tag_failures))
}
