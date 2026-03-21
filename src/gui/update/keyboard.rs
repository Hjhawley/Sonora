//! gui/update/keyboard.rs
//! Global keyboard shortcuts and keyboard navigation.
//!
//! Current behavior:
//! - Up / Down: move track selection
//! - Shift+Up / Shift+Down: extend selection range
//! - Left / Right: previous / next track during playback
//! - Ctrl+A: select all tracks in the current context
//! - Enter: save inspector edits (if dirty)
//! - Escape: clear selection / close inspector
//! - Space: toggle play / pause
//!
//! This is intentionally global and simple for now.
//! If you later add explicit focus tracking, this is the file to refine.

use iced::Task;
use iced::keyboard::{self, Key, key::Named};

use super::super::state::{Message, Sonora};
use super::playback;
use super::save;
use super::selection;

pub(crate) fn handle_event(state: &mut Sonora, event: keyboard::Event) -> Task<Message> {
    match event {
        keyboard::Event::ModifiersChanged(modifiers) => {
            state.modifiers = modifiers;
            Task::none()
        }

        keyboard::Event::KeyPressed {
            key,
            modified_key,
            text,
            ..
        } => {
            let _ = text; // intentionally ignored for global shortcuts
            handle_key_pressed(state, key, modified_key)
        }

        _ => Task::none(),
    }
}

fn handle_key_pressed(state: &mut Sonora, key: Key, modified_key: Key) -> Task<Message> {
    let shift = state.modifiers.shift();
    let ctrl = state.modifiers.control();

    if ctrl {
        if let Key::Character(s) = &modified_key {
            if s.eq_ignore_ascii_case("a") {
                return selection::select_all_in_context(state);
            }
        }
    }

    match key {
        Key::Named(Named::ArrowUp) => selection::select_adjacent_track(state, -1, shift),
        Key::Named(Named::ArrowDown) => selection::select_adjacent_track(state, 1, shift),

        Key::Named(Named::ArrowLeft) => playback::prev(state),
        Key::Named(Named::ArrowRight) => playback::next(state),

        Key::Named(Named::Enter) => {
            if state.inspector_dirty {
                save::save_inspector_to_file(state)
            } else {
                Task::none()
            }
        }

        Key::Named(Named::Escape) => selection::clear_selection(state),

        Key::Named(Named::Space) => playback::toggle_play_pause(state),

        _ => Task::none(),
    }
}
