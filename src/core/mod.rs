//! core/mod.rs
//!
//! Public core surface for Sonora.
//!
//! Architecture:
//! - scans discover filesystem candidates
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
    load_hidden_tracks_from_db, load_missing_tracks_from_db, load_visible_tracks_from_db,
};
pub use scan::scan_paths;
