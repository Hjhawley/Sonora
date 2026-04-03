//! gui/update/query.rs
//!
//! Query/search/sort update handlers.

use iced::Task;

use super::super::query::{SortDirection, TrackSortField};
use super::super::state::{Message, Sonora};

pub(crate) fn track_search_changed(state: &mut Sonora, value: String) -> Task<Message> {
    state.track_query.search_text = value;

    // Search can shrink the visible list dramatically.
    // Reset scroll so virtualization doesn't try to stay deep in the old range.
    state.tracks_scroll_offset_y = 0.0;

    state.rebuild_track_query_caches();
    Task::none()
}

pub(crate) fn clear_track_search(state: &mut Sonora) -> Task<Message> {
    if state.track_query.search_text.is_empty() {
        return Task::none();
    }

    state.track_query.search_text.clear();

    // Clearing search changes the visible list again; reset to top.
    state.tracks_scroll_offset_y = 0.0;

    state.rebuild_track_query_caches();
    Task::none()
}

pub(crate) fn set_track_sort_field(state: &mut Sonora, field: TrackSortField) -> Task<Message> {
    if state.track_query.sort_field == field {
        state.track_query.sort_direction = state.track_query.sort_direction.toggled();
    } else {
        state.track_query.sort_field = field;
        state.track_query.sort_direction = SortDirection::Asc;
    }

    // sort from the top
    state.tracks_scroll_offset_y = 0.0;

    state.rebuild_track_query_caches();
    Task::none()
}
