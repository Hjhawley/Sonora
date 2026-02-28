//! core/tags/mod.rs
//!
//! Metadata IO boundary (tag read/write + art extraction).
//!
//! Public surface area is intentionally small:
//! - `read_track_row(path) -> (TrackRow, failed)`
//! - `write_track_row(row, write_extended) -> Result<(), String>`
//! - `read_embedded_art(path) -> Result<Option<(bytes, mime)>, String>`
//!
//! Everything below this layer is "tag-format-specific" (ID3 today).
//! The rest of the app should treat this as a pluggable backend.

mod art;
mod read;
mod util;
mod write;

pub use art::read_embedded_art;
pub use read::read_track_row;
pub use write::write_track_row;
