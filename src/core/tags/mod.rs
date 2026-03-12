//! core/tags/mod.rs
//! Metadata IO boundary (tag read/write + art extraction).
//!
//! Public surface area is intentionally small:
//! - `read_track_row(path) -> (TrackRow, failed)`
//! - `write_track_row(row, write_extended) -> Result<(), String>`
//! - `read_embedded_art(path) -> Result<Option<(bytes, mime)>, String>`
//! - crate-visible release-date helpers re-exported for higher layers that need
//!   the same normalization rules as the tag backend
//!
//! Current implementation is ID3-only.
//! The rest of the app should treat this module as the metadata backend so
//! future formats can be added without changing higher layers.

mod art;
mod read;
mod util;
mod write;

pub use art::read_embedded_art;
pub use read::read_track_row;
pub use write::write_track_row;

pub(crate) use util::{extract_year_from_release_date, normalize_release_date};