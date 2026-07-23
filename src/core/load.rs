//! core/load.rs
//!
//! DB-backed 'TrackRow' loading helpers.
//!
//! These functions reconstruct cached rows directly from SQLite without
//! rescanning the filesystem or rereading media tags. This is the normal path
//! for startup and library-scope transitions.

use crate::core::db;
use crate::core::types::TrackRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadScope {
    Visible,
    Hidden,
    Missing,
}

/// Load cached track rows for a library scope.
///
/// This is the preferred core loading API. A DB-backed load performs no
/// hydration and therefore has no hydration-failure count.
pub fn load_track_rows_from_db(scope: LoadScope) -> Result<Vec<TrackRow>, String> {
    let db_path = db::default_db_path()?;
    let db = db::Db::open(&db_path)?;

    match scope {
        LoadScope::Visible => db.load_visible_tracks(),
        LoadScope::Hidden => db.load_hidden_tracks(),
        LoadScope::Missing => db.load_missing_tracks(),
    }
}

/// Compatibility adapter for GUI flows that currently share the scan result
/// shape '(rows, hydration_failures)'.
///
/// The second value is always zero because this path does not read files.
pub fn load_tracks_from_db(scope: LoadScope) -> Result<(Vec<TrackRow>, usize), String> {
    let rows = load_track_rows_from_db(scope)?;
    Ok((rows, 0))
}

pub fn load_visible_tracks_from_db() -> Result<(Vec<TrackRow>, usize), String> {
    load_tracks_from_db(LoadScope::Visible)
}

pub fn load_hidden_tracks_from_db() -> Result<(Vec<TrackRow>, usize), String> {
    load_tracks_from_db(LoadScope::Hidden)
}

pub fn load_missing_tracks_from_db() -> Result<(Vec<TrackRow>, usize), String> {
    load_tracks_from_db(LoadScope::Missing)
}
