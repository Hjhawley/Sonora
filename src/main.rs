//! Sonora GUI
//!
//! # What this program is
//! A small desktop app (built with the 'iced' GUI library) that scans folders for '.mp3' files,
//! reads ID3 tags (title/artist/album/etc), and shows them in a UI.
//!
//! # How Iced works
//! - 'Sonora' = the *entire memory* of the app (all the state)
//! - 'Message' = "something happened" (button clicked, typed a letter, scan finished)
//! - 'update(state, message)' = handles that thing and updates state
//! - 'view(state)' = draws UI based on the current state
//!
//! The app repeats this forever:
//! **Message happens -> update changes state -> view redraws**
//!
//! # Current behavior (read-only)
//! - User adds one or more folder roots.
//! - "Scan Library" walks roots for '.mp3', reads ID3 into 'TrackRow'.
//! - Library is displayed as either:
//!   - Track View: flat list of tracks
//!   - Album View: grouped by (artist, album), with expandable albums
//! - Selecting a track populates the Inspector form.
//! - "Save edits" updates the in-memory 'TrackRow' only (no disk writes yet)
//!
//! # Not implemented yet
//! - Writing tags back to files
//! - Persistent DB/cache
//! - Audio playback
//!
//! # Architecture constraints (on purpose)
//! - UI layer calls 'core::*' for scanning/tag reading.
//! - UI does not perform filesystem IO except validating user-entered root paths.
//!
//! # Concurrency model (aka "don't freeze the app")
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
