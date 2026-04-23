//! core/scan.rs
//!
//! Library scan orchestration.
//! - discover candidate audio files under the configured roots
//! - reconcile discovered filesystem state into SQLite
//! - hydrate metadata only for changed/new rows
//! - persist refreshed metadata
//! - reload the requested library scope from the DB

use std::collections::HashSet;
use std::path::PathBuf;

use crate::core::db;
use crate::core::hydrate_tracks;
use crate::core::library::{self, DiscoveredFile};
use crate::core::load::LoadScope;
use crate::core::types::TrackRow;

/// Run the full scan/reconcile/refresh pipeline, then load the requested scope.
pub fn run_scan_for_scope(
    roots: &[PathBuf],
    scope: LoadScope,
) -> Result<(Vec<TrackRow>, usize), String> {
    let discovered = scan_paths(roots)?;

    let db_path = db::default_db_path()?;
    let mut db = db::Db::open(&db_path)?;

    let changed_id_paths = db.upsert_discovered(roots, &discovered)?;

    let tag_failures = if changed_id_paths.is_empty() {
        0usize
    } else {
        let (rows, failures) = hydrate_tracks(changed_id_paths);
        db.upsert_track_rows_metadata(&rows)?;
        failures
    };

    let rows = match scope {
        LoadScope::Visible => db.load_visible_tracks()?,
        LoadScope::Hidden => db.load_hidden_tracks()?,
        LoadScope::Missing => db.load_missing_tracks()?,
    };

    Ok((rows, tag_failures))
}

/// Discover candidate audio files under multiple roots.
/// - de-dupes across overlapping roots by full path
/// - sorts paths once so downstream code gets stable ordering
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
