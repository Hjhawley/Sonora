//! main.rs
//!
//! Current behavior
//! - User adds one or more library root folders.
//! - "Scan Library" walks roots for `.mp3` files and reads ID3 tags into `TrackRow`.
//! - Library can be viewed as:
//!   - Track View: flat list
//!   - Album View: grouped by (album artist, album) with expandable album rows
//! - Selecting a track loads an Inspector (draft fields).
//! - "Save edits" writes the edited ID3 tags back to that single file, then re-reads it.
//! - Audio playback
//!
//! Future behavior
//! - Persistent cache / DB
//! - Multi-file batch editing

#![forbid(unsafe_code)]

mod core;
mod gui;

use crate::gui::{Sonora, update, view};

fn main() -> iced::Result {
    iced::application(Sonora::default, update, view)
        .title("Sonora")
        .run()
}
