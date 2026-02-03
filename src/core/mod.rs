//! Sonora core module (non-UI logic).
//!
//! Think of 'core' as: **"the brain"** (filesystem scan + tag reading),
//! while the GUI is just: **"the face"** (buttons, lists, text inputs).
//!
//! Why split core vs UI?
//! - UI code stays simple and less error-prone
//! - core logic becomes easier to test
//! - later we can reuse core for:
//!   - a CLI tool
//!   - a different GUI
//!   - a background scanner service
//!
//! In other words:
//! - The UI should *ask* for data ("scan these folders")
//! - core should *return* plain structs ("here are the tracks")

pub mod library;
pub mod tags;
pub mod types;

use std::collections::HashSet;
use std::path::PathBuf;

use types::TrackRow;

/// Scan one or more folder roots for '.mp3' files, then read ID3 tags for each file.
///
/// Big picture steps:
/// 1) For each root folder, find every '.mp3' under it (library::scan_mp3s)
/// 2) For each file path, read ID3 tags into a TrackRow (tags::read_track_row)
/// 3) Return all TrackRows + a count of how many tag reads failed
///
/// Return type:
/// - 'Ok((Vec<TrackRow>, usize))'
///    - Vec<TrackRow> = one "row" per file (path + metadata)
///    - usize = how many tag reads failed (still returns rows, just missing metadata)
/// - 'Err(String)' if filesystem scanning failed (like permissions / missing folder)
///
/// Why do we count tag failures instead of erroring out?
/// - A library can have a few broken files. We still want to load the rest.
///
/// Why do we use HashSet?
/// - If the user adds overlapping roots, we don't want duplicates.
///   Example:
///   - Root A: 'D:\Music'
///   - Root B: 'D:\Music\Soundtracks'
///   Those overlap, so without dedupe, we'd double-count files.
pub fn scan_and_read_roots(roots: Vec<PathBuf>) -> Result<(Vec<TrackRow>, usize), String> {
    let mut rows = Vec::new();
    let mut tag_failures = 0usize;

    // HashSet lets us ask "have I seen this path already?" in fast time.
    let mut seen = HashSet::<PathBuf>::new();

    for root in roots {
        // 1) Find all mp3 paths under this root folder.
        // '?' = bubble up the error if scanning fails.
        let paths = library::scan_mp3s(&root)?;

        // 2) Read tags for each path and convert to TrackRow.
        for path in paths {
            // insert() returns false if it was already present.
            if !seen.insert(path.clone()) {
                continue; // skip duplicates
            }

            // tags::read_track_row never panics and never errors out the whole scan.
            // It returns (row, failed?)
            let (row, failed) = tags::read_track_row(path);

            if failed {
                tag_failures += 1;
            }

            rows.push(row);
        }
    }

    Ok((rows, tag_failures))
}
