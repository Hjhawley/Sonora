//! core/tags/mod.rs
//! 
//! ID3 tag read/write utilities.
//! Public API:
//! - [`read_track_row`] reads an MP3 into a [`TrackRow`] (non-fatal on tag read failure).
//! - [`write_track_row`] writes selected fields back to disk.

mod art;
mod read;
mod util;
mod write;

pub use read::read_track_row;
pub use write::write_track_row;
pub use art::read_embedded_art;