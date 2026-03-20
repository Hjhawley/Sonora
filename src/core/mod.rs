//! core/mod.rs
//!
//! The brain of the app
//! - scan: discover files + hydrate only changed files
//! - startup/scope loads: build TrackRows directly from DB

pub mod db;
pub mod library;
pub mod playback;
pub mod probe;
pub mod tags;
pub mod types;

use std::collections::HashSet;
use std::path::PathBuf;

use library::DiscoveredFile;
use types::{TrackId, TrackRow};

/// Discover candidate audio files under multiple roots.
///
/// - Currently MP3-only for MVP (library enforces extension rules)
/// - De-dupes across overlapping roots by full path
/// - Sorts paths once (core owns ordering; GUI shouldn't)
pub fn scan_paths(roots: &[PathBuf]) -> Result<Vec<DiscoveredFile>, String> {
    let mut seen: HashSet<PathBuf> = HashSet::with_capacity(1024);
    let mut out: Vec<DiscoveredFile> = Vec::new();

    for root in roots {
        let files = library::scan_audio_files(root)?;
        for file in files {
            if seen.insert(file.path.clone()) {
                out.push(file);
            }
        }
    }

    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Hydrate real on-disk metadata for a DB-backed `(TrackId, PathBuf)` list.
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

/// Load visible library rows from the DB without scanning for filesystem updates.
pub fn load_visible_tracks_from_db() -> Result<(Vec<TrackRow>, usize), String> {
    let db_path = db::default_db_path()?;
    let db = db::Db::open(&db_path)?;
    let rows = db.load_visible_tracks()?;
    Ok((rows, 0))
}

/// Load hidden library rows from the DB without scanning for filesystem updates.
pub fn load_hidden_tracks_from_db() -> Result<(Vec<TrackRow>, usize), String> {
    let db_path = db::default_db_path()?;
    let db = db::Db::open(&db_path)?;
    let rows = db.load_hidden_tracks()?;
    Ok((rows, 0))
}

/// Load missing library rows from the DB without scanning for filesystem updates.
pub fn load_missing_tracks_from_db() -> Result<(Vec<TrackRow>, usize), String> {
    let db_path = db::default_db_path()?;
    let db = db::Db::open(&db_path)?;
    let rows = db.load_missing_tracks()?;
    Ok((rows, 0))
}
