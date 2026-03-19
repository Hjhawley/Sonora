//! gui/update/query.rs
//!
//! Query/search/sort update handlers.

use iced::Task;

use super::super::query::{SortDirection, TrackSortField};
use super::super::state::{Message, Sonora};

pub(crate) fn track_search_changed(state: &mut Sonora, value: String) -> Task<Message> {
    state.track_query.search_text = value;
    Task::none()
}

pub(crate) fn clear_track_search(state: &mut Sonora) -> Task<Message> {
    if state.track_query.search_text.is_empty() {
        return Task::none();
    }

    state.track_query.search_text.clear();
    Task::none()
}

pub(crate) fn set_track_sort_field(state: &mut Sonora, field: TrackSortField) -> Task<Message> {
    if state.track_query.sort_field == field {
        state.track_query.sort_direction = state.track_query.sort_direction.toggled();
    } else {
        state.track_query.sort_field = field;
        state.track_query.sort_direction = SortDirection::Asc;
    }

    Task::none()
}

pub(crate) fn toggle_track_sort_direction(state: &mut Sonora) -> Task<Message> {
    state.track_query.sort_direction = state.track_query.sort_direction.toggled();
    Task::none()
}
