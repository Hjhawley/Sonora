//! Update logic (router).
//! Mutates state in response to `Message` events.

use iced::Task;

use super::state::{Message, Sonora};

mod helpers;
mod inspector;
mod roots;
mod save;
mod scan;
mod selection;

pub(crate) fn update(state: &mut Sonora, message: Message) -> Task<Message> {
    match message {
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
        Message::SelectTrack(i) => selection::select_track(state, i),

        // Cover
        Message::CoverLoaded(i, handle) => selection::cover_loaded(state, i, handle),

        // Inspector
        Message::ToggleExtended(v) => inspector::toggle_extended(state, v),
        Message::InspectorChanged(field, value) => {
            inspector::inspector_changed(state, field, value)
        }

        // Save
        Message::SaveInspectorToFile => save::save_inspector_to_file(state),
        Message::SaveFinished(i, result) => save::save_finished(state, i, result),
        Message::SaveFinishedBatch(result) => save::save_finished_batch(state, result),
        Message::RevertInspector => save::revert_inspector(state),
    }
}
