//! core/mod.rs
//!
//! - scan flows discover filesystem candidates, reconcile them into SQLite,
//!   refresh metadata only for changed files, and reload the requested scope
//! - hydration reads real metadata from disk for selected files
//! - DB-backed loads build TrackRows directly from SQLite for startup/scope switches

pub mod db;
pub mod hydrate;
pub mod library;
pub mod load;
pub mod playback;
pub mod probe;
pub mod scan;
pub mod tags;
pub mod types;

pub use hydrate::hydrate_tracks;
pub use load::{
    LoadScope, load_hidden_tracks_from_db, load_missing_tracks_from_db, load_visible_tracks_from_db,
};
pub use scan::run_scan_for_scope;
