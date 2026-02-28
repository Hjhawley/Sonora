//! core/mod.rs
//!
//! The brain of the app:
//! - Discover candidate audio file paths (filesystem walk)
//! - Read/write tags (metadata IO)
//! - Return plain data structs for the GUI to render
//!
//! - Make the scan pipeline explicit and modular:
//!   (A) discover paths -> Vec<PathBuf>
//!   (B) read tags -> Vec<TrackRow>
//!
//! This keeps the GUI dumb, and makes the later SQLite pivot easy:
//! - "scan" becomes "discover paths -> upsert/load from DB"
//! - but (A) and (B) remain stable APIs.

pub mod library;
pub mod playback;
pub mod tags;
pub mod types;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use types::TrackRow;

/// Discover candidate audio files under multiple roots.
///
/// - MP3-only for MVP (library enforces extension rules)
/// - De-dupes across overlapping roots by full path
/// - Sorts paths once (core owns ordering, GUI shouldn't)
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

/// Convenience: old API preserved (GUI can keep calling this for now).
///
/// Internally, this is now just:
/// - scan_paths(roots)
/// - read_tracks(paths)
pub fn scan_and_read_roots(roots: &[PathBuf]) -> Result<(Vec<TrackRow>, usize), String> {
    let paths = scan_paths(roots)?;
    let (rows, failures) = read_tracks(paths);
    Ok((rows, failures))
}

/// Convenience for callers that have a single root.
pub fn scan_paths_one(root: &Path) -> Result<Vec<PathBuf>, String> {
    scan_paths(&[root.to_path_buf()])
}
