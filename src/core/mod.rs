//! core/mod.rs
//!
//! The brain of the app:
//! - Discover candidate audio file paths (filesystem walk)
//! - Read/write tags (metadata IO)
//! - Return plain data structs for the GUI to render
//!
//! Scan / load pipeline boundary:
//!   (A) discover paths
//!   (B) DB-backed identity + visibility
//!   (C) hydrate TrackRows from file paths

pub mod db;
pub mod library;
pub mod playback;
pub mod tags;
pub mod types;

use std::collections::HashSet;
use std::path::PathBuf;

use library::DiscoveredFile;
use types::{TrackId, TrackRow};

/// Discover candidate audio files under multiple roots.
///
/// - MP3-only for MVP (library enforces extension rules)
/// - De-dupes across overlapping roots by full path
/// - Sorts paths once (core owns ordering; GUI shouldn't)
pub fn scan_paths(roots: &[PathBuf]) -> Result<Vec<DiscoveredFile>, String> {
    let mut seen: HashSet<PathBuf> = HashSet::with_capacity(1024);
    let mut out: Vec<DiscoveredFile> = Vec::new();

    for root in roots {
        let files = library::scan_mp3s(root)?;
        for file in files {
            if seen.insert(file.path.clone()) {
                out.push(file);
            }
        }
    }

    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Read tags for a set of DB-backed `(TrackId, PathBuf)` pairs.
///
/// - Never fails hard per-file: unreadable tags return an "empty-ish" TrackRow
/// - Ensures every returned row has its DB-owned TrackId attached
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

/// Load visible library rows from the DB without scanning for filesystem updates.
///
/// This means:
/// - startup can restore the previous library immediately
/// - explicit Scan is still what refreshes `present`/missing status
pub fn load_visible_tracks_from_db() -> Result<(Vec<TrackRow>, usize), String> {
    let db_path = db::default_db_path()?;
    let db = db::Db::open(&db_path)?;
    let id_paths = db.load_visible_paths()?;
    Ok(hydrate_tracks(id_paths))
}
