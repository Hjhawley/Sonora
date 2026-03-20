//! gui/update/mod.rs
//! Update logic (router).
//!
//! Route by TrackId, never by Vec index.

use iced::Task;

use super::state::{Message, Sonora};

mod inspector;
mod keyboard;
mod playback;
mod query;
mod roots;
mod save;
mod scan;
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
        Message::SetLibraryScope(scope) => selection::set_library_scope(state, scope),
        Message::ScopeLoaded(result) => selection::scope_loaded(state, result),

        // Track View query / sorting
        Message::TrackSearchChanged(value) => query::track_search_changed(state, value),
        Message::ClearTrackSearch => query::clear_track_search(state),
        Message::SetTrackSortField(field) => query::set_track_sort_field(state, field),
        Message::ToggleTrackSortDirection => query::toggle_track_sort_direction(state),
        Message::SetTrackSortDirection(dir) => {
            state.track_query.sort_direction = dir;
            state.rebuild_track_query_caches();
            Task::none()
        }
        Message::TracksScrolled {
            offset_y,
            viewport_height,
        } => {
            state.tracks_scroll_offset_y = offset_y.max(0.0);
            state.tracks_viewport_height = viewport_height.max(0.0);
            Task::none()
        }

        // View + selection
        Message::SetViewMode(mode) => selection::set_view_mode(state, mode),
        Message::SelectAlbum(key) => selection::select_album(state, key),
        Message::SelectTrack(id) => selection::select_track(state, id),
        Message::TrackPressed(id) => selection::track_pressed(state, id),
        Message::AlbumTilePressed(key) => selection::album_tile_pressed(state, key),
        Message::AlbumHeaderPressed(key) => selection::album_header_pressed(state, key),
        Message::AlbumTrackPressed(key, id) => selection::album_track_pressed(state, key, id),
        Message::ClearSelection => selection::clear_selection(state),

        // Cover
        Message::CoverLoaded(id, handle) => selection::cover_loaded(state, id, handle),

        // Playback
        Message::PlaySelected => playback::play_selected(state),
        Message::PlayTrack(id) => playback::play_track(state, id),
        Message::PlayAlbum(key) => playback::play_album(state, key),
        Message::PlayAlbumFromTrack(key, id) => playback::play_album_from_track(state, key, id),
        Message::TogglePlayPause => playback::toggle_play_pause(state),
        Message::ToggleShuffle => playback::toggle_shuffle(state),
        Message::CycleRepeatMode => playback::cycle_repeat_mode(state),
        Message::Next => playback::next(state),
        Message::Prev => playback::prev(state),

        Message::SeekTo(ratio) => playback::seek_preview(state, ratio),
        Message::SeekCommit => playback::seek_commit(state),
        Message::SetVolume(vol) => playback::set_volume(state, vol),

        Message::PlaybackEvent(ev) => playback::handle_event(state, ev),

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
        Message::HideSelected => selection::hide_selected(state),
        Message::UnhideSelected => selection::unhide_selected(state),
        Message::DeleteSelectedFromSonora => selection::delete_selected_from_sonora(state),
    }
}
