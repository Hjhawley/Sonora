//! Sonora GUI prototype
//!
//! Current behavior (read-only)
//! - User adds one or more folder roots.
//! - "Scan Library" walks roots for '.mp3', reads ID3 into 'TrackRow'.
//! - Library is displayed as either:
//!   - Track View: flat list of tracks
//!   - Album View: grouped by (artist, album), with expandable albums
//! - Selecting a track populates the Inspector form.
//! - "Save edits" updates the in-memory 'TrackRow' only (no disk writes yet)
//!
//! Not yet implemented
//! - Writing tags back to files
//! - Persistent DB/cache
//! - Audio playback
//!
//! Architecture constraints
//! - UI layer calls 'core::*' for scanning/tag reading.
//! - UI does not perform filesystem IO except validating user-entered root paths.
//!
//! Concurrency model
//! - Scanning the disk can be slow.
//! - So we run scan work on a separate thread.
//! - When it finishes, it sends the results back as a 'Message::ScanFinished(...)'
//!   so 'update()' can safely apply the result.

mod core;
mod gui;

use crate::gui::{Sonora, update, view};

fn main() -> iced::Result {
    iced::application(Sonora::default, update, view)
        .title("Sonora")
        .run()
}
