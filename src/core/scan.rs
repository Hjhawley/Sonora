//! core/scan.rs
//!
//! Library scan orchestration.
//!
//! The scan pipeline:
//! 1. discovers candidate audio files under all configured roots
//! 2. de-duplicates overlapping-root results
//! 3. reconciles filesystem state into SQLite
//! 4. hydrates only new, changed, or stale-cache rows
//! 5. persists only successfully hydrated metadata
//! 6. reloads the requested library scope from SQLite

use std::collections::HashSet;
use std::path::PathBuf;

use crate::core::db;
use crate::core::hydrate_tracks;
use crate::core::library::{self, DiscoveredFile};
use crate::core::load::LoadScope;
use crate::core::types::TrackRow;

/// Run the complete discovery, reconciliation, hydration, persistence, and
/// scope-loading pipeline.
///
/// The returned failure count includes files selected for hydration whose
/// on-disk metadata could not be read reliably. Failed rows remain stale in
/// SQLite and will be retried during a later scan.
pub fn run_scan_for_scope(
    roots: &[PathBuf],
    scope: LoadScope,
) -> Result<(Vec<TrackRow>, usize), String> {
    // Complete filesystem discovery before opening a reconciliation
    // transaction. A discovery failure therefore leaves cached presence state
    // unchanged.
    let discovered = scan_paths(roots)?;

    let db_path = db::default_db_path()?;
    let mut db = db::Db::open(&db_path)?;

    let tracks_to_hydrate = db.upsert_discovered(roots, &discovered)?;

    let (hydrated_rows, hydration_failures) = hydrate_tracks(tracks_to_hydrate);

    if !hydrated_rows.is_empty() {
        db.update_track_rows_metadata(&hydrated_rows)?;
    }

    let rows = match scope {
        LoadScope::Visible => db.load_visible_tracks()?,
        LoadScope::Hidden => db.load_hidden_tracks()?,
        LoadScope::Missing => db.load_missing_tracks()?,
    };

    Ok((rows, hydration_failures))
}

/// Discover candidate audio files under multiple roots.
///
/// Results are:
/// - de-duplicated by lexical full path
/// - stable-sorted by full path
///
/// Discovery completes for every root before reconciliation begins. If any
/// root cannot be scanned completely, the whole operation returns an error.
pub fn scan_paths(roots: &[PathBuf]) -> Result<Vec<DiscoveredFile>, String> {
    let mut seen = HashSet::with_capacity(1024);
    let mut discovered = Vec::new();

    for root in roots {
        for file in library::scan_audio_files(root)? {
            if seen.insert(file.path.clone()) {
                discovered.push(file);
            }
        }
    }

    discovered.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(discovered)
}
