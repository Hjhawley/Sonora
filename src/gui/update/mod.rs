//! gui/update/mod.rs
//! Update logic (router).
//!
//! route by TrackId, never by Vec index.
//! This module should stay dumb: it just dispatches messages to the right handler.

use iced::Task;

use super::state::{Message, Sonora};

mod inspector;
mod playback;
mod roots;
mod save;
mod scan;
mod selection;
mod util;

pub(crate) fn update(state: &mut Sonora, message: Message) -> Task<Message> {
    match message {
        Message::Noop => Task::none(),

        Message::TickPlayback => playback::drain_events(state),

        // Roots
        Message::RootInputChanged(s) => roots::root_input_changed(state, s),
        Message::AddRootPressed => roots::add_root_pressed(state),
        Message::RemoveRoot(i) => roots::remove_root(state, i),

        // Scan
        Message::ScanLibrary => scan::scan_library(state),
        Message::ScanFinished(result) => scan::scan_finished(state, result),

        // View + selection
        Message::SetViewMode(mode) => selection::set_view_mode(state, mode),
        Message::SelectAlbum(key) => selection::select_album(state, key),
        Message::SelectTrack(id) => selection::select_track(state, id),

        // Cover
        Message::CoverLoaded(id, handle) => selection::cover_loaded(state, id, handle),

        // Playback
        Message::PlaySelected => playback::play_selected(state),
        Message::PlayTrack(id) => playback::play_track(state, id),
        Message::TogglePlayPause => playback::toggle_play_pause(state),
        Message::Next => playback::next(state),
        Message::Prev => playback::prev(state),

        // Seek: preview vs commit
        Message::SeekTo(ratio) => playback::seek_preview(state, ratio),
        Message::SeekCommit => playback::seek_commit(state),

        Message::SetVolume(vol) => playback::set_volume(state, vol),

        // Playback (optional path)
        Message::PlaybackEvent(ev) => playback::handle_event(state, ev),

        // Inspector
        Message::ToggleExtended(v) => inspector::toggle_extended(state, v),
        Message::InspectorChanged(field, value) => {
            inspector::inspector_changed(state, field, value)
        }

        // Save
        Message::SaveInspectorToFile => save::save_inspector_to_file(state),
        Message::SaveFinished(id, result) => save::save_finished(state, id, result),
        Message::SaveFinishedBatch(result) => save::save_finished_batch(state, result),
        Message::RevertInspector => save::revert_inspector(state),
    }
}
