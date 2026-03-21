//! core/load.rs
//!
//! DB-backed TrackRow loading helpers.
//!
//! These helpers load cached TrackRows directly from SQLite without scanning
//! the filesystem, which keeps startup and scope switches fast.

use crate::core::db;
use crate::core::types::TrackRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadScope {
    Visible,
    Hidden,
    Missing,
}

pub fn load_tracks_from_db(scope: LoadScope) -> Result<(Vec<TrackRow>, usize), String> {
    let db_path = db::default_db_path()?;
    let db = db::Db::open(&db_path)?;

    let rows = match scope {
        LoadScope::Visible => db.load_visible_tracks()?,
        LoadScope::Hidden => db.load_hidden_tracks()?,
        LoadScope::Missing => db.load_missing_tracks()?,
    };

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
