//! Sonora core module (non-UI logic).
//!
//! Goal: keep "business logic" out of the GUI.
//! - The UI should NOT walk the filesystem directly.
//! - The UI should NOT parse ID3 tags directly.
//!
//! Instead, the UI calls into `core` functions (like `scan_and_read_roots`),
//! and core returns plain Rust structs (like `TrackRow`) the UI can display.
//!
//! This separation is important because:
//! - It keeps the UI simple.
//! - It makes the logic easier to test.
//! - Later we can reuse core for a CLI, different UI, etc.

pub mod library;
pub mod tags;
pub mod types;

use std::collections::HashSet;
use std::path::PathBuf;

use types::TrackRow;

/// Scan one or more folder roots for `.mp3` files, then read ID3 tags for each file.
///
/// Returns:
/// - `Vec<TrackRow>`: one row per track (file path + metadata fields we care about)
/// - `usize`: number of tag read failures (corrupt/missing tags, etc)
///
/// Notes:
/// - This does NOT write anything to disk. Read-only.
/// - Duplicate files are skipped if roots overlap (using a `HashSet`).
/// - Errors in tag reading should not crash the whole scan; we count failures instead.
pub fn scan_and_read_roots(roots: Vec<PathBuf>) -> Result<(Vec<TrackRow>, usize), String> {
    let mut rows = Vec::new();
    let mut tag_failures = 0usize;

    // Tracks which files we've already seen so overlapping roots don't duplicate work.
    let mut seen = HashSet::<PathBuf>::new();

    for root in roots {
        // 1) Find all mp3 paths under this root folder
        let paths = library::scan_mp3s(&root)?;

        // 2) Read tags for each path and convert to TrackRow
        for path in paths {
            if !seen.insert(path.clone()) {
                continue; // avoid duplicates if roots overlap
            }

            let (row, failed) = tags::read_track_row(path);
            if failed {
                tag_failures += 1;
            }
            rows.push(row);
        }
    }

    Ok((rows, tag_failures))
}
