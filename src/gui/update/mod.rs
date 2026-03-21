//! gui/update/mod.rs
//! Update logic (router).
//!
//! Route by TrackId, never by Vec index.

use iced::Task;

use super::state::{Message, Sonora};

mod actions;
mod art;
mod inspector;
mod keyboard;
mod playback;
mod query;
mod roots;
mod save;
mod scan;
mod scope;
mod selection;
mod util;

pub(crate) fn update(state: &mut Sonora, message: Message) -> Task<Message> {
    match message {
        Message::Noop => Task::none(),

        Message::TickPlayback => playback::drain_events(state),

        // Global keyboard
        Message::KeyboardEvent(event) => keyboard::handle_event(state, event),

        // Roots
        Message::RootInputChanged(s) => roots::root_input_changed(state, s),
        Message::AddRootPressed => roots::add_root_pressed(state),
        Message::RemoveRoot(i) => roots::remove_root(state, i),

        // Scan
        Message::ScanLibrary => scan::scan_library(state),
        Message::ScanFinished(result) => scan::scan_finished(state, result),

        // Scope / library dataset
        Message::SetLibraryScope(scope_value) => scope::set_library_scope(state, scope_value),
        Message::ScopeLoaded(result) => scope::scope_loaded(state, result),

        // Track View query / sorting
        Message::TrackSearchChanged(value) => query::track_search_changed(state, value),
        Message::ClearTrackSearch => query::clear_track_search(state),
        Message::SetTrackSortField(field) => query::set_track_sort_field(state, field),
        Message::TracksScrolled {
            offset_y,
            viewport_height,
        } => {
            state.tracks_scroll_offset_y = offset_y.max(0.0);
            state.tracks_viewport_height = viewport_height.max(0.0);
            Task::none()
        }

        // View + selection
        Message::SetViewMode(mode) => scope::set_view_mode(state, mode),
        Message::TrackPressed(id) => selection::track_pressed(state, id),

        // Album-view click handling
        Message::AlbumTilePressed(key) => selection::album_tile_pressed(state, key),
        Message::AlbumHeaderPressed(key) => selection::album_header_pressed(state, key),
        Message::AlbumTrackPressed(key, id) => selection::album_track_pressed(state, key, id),

        // Cover
        Message::CoverLoaded(id, handle) => selection::cover_loaded(state, id, handle),

        // Inspector artwork
        Message::ChooseInspectorArtwork => art::choose_inspector_artwork(state),
        Message::InspectorArtworkChosen(result) => art::inspector_artwork_chosen(state, result),
        Message::RemoveInspectorArtwork => art::remove_inspector_artwork(state),
        Message::ExtractInspectorArtwork => art::extract_inspector_artwork(state),
        Message::InspectorArtworkExtracted(result) => {
            art::inspector_artwork_extracted(state, result)
        }

        // Playback
        Message::PlayTrack(id) => playback::play_track(state, id),
        Message::PlayAlbum(key) => playback::play_album(state, key),
        Message::TogglePlayPause => playback::toggle_play_pause(state),
        Message::ToggleShuffle => playback::toggle_shuffle(state),
        Message::CycleRepeatMode => playback::cycle_repeat_mode(state),
        Message::Next => playback::next(state),
        Message::Prev => playback::prev(state),

        Message::SeekTo(ratio) => playback::seek_preview(state, ratio),
        Message::SeekCommit => playback::seek_commit(state),
        Message::SetVolume(vol) => playback::set_volume(state, vol),

        // Inspector
        Message::InspectorChanged(field, value) => {
            inspector::inspector_changed(state, field, value)
        }

        // Save
        Message::SaveInspectorToFile => save::save_inspector_to_file(state),
        Message::SaveFinished(id, result) => save::save_finished(state, id, result),
        Message::SaveFinishedBatch(result) => save::save_finished_batch(state, result),
        Message::RevertInspector => save::revert_inspector(state),

        // Sonora-only visibility / DB record actions
        Message::HideSelected => actions::hide_selected(state),
        Message::UnhideSelected => actions::unhide_selected(state),
        Message::DeleteSelectedFromSonora => actions::delete_selected_from_sonora(state),
    }
}
