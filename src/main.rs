//! main.rs
//!
//! Current behavior
//! - User adds one or more library root folders.
//! - "Scan Library" walks roots for '.mp3' files and reads ID3 tags into 'TrackRow'.
//! - Library can be viewed as:
//!   - Track View: flat list
//!   - Album View: grouped by (album artist, album) with expandable album rows
//! - Selecting a track loads an Inspector (draft fields).
//! - "Save edits" writes the edited ID3 tags back to that single file, then re-reads it.
//! - Audio playback
//! - Persistent cache / DB

#![forbid(unsafe_code)]

mod core;
mod gui;

use iced::{Size, Theme, window};

use crate::gui::theme::{APP_FONT, APP_FONT_BYTES, app_theme};
use crate::gui::view::constants::{WINDOW_H, WINDOW_W};
use crate::gui::{Sonora, subscription, update, view};

fn sonora_theme(_: &Sonora) -> Theme {
    app_theme()
}

fn main() -> iced::Result {
    iced::application(Sonora::default, update, view)
        .title("Sonora")
        .subscription(subscription)
        .theme(sonora_theme)
        .font(APP_FONT_BYTES)
        .default_font(APP_FONT)
        .window(window::Settings {
            size: Size::new(WINDOW_W, WINDOW_H),
            min_size: Some(Size::new(720.0, 540.0)),
            resizable: true,
            ..Default::default()
        })
        .run()
}
