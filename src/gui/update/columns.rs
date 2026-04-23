//! gui/update/columns.rs
//!
//! Track View column layout interactions.
//! TODO: For now this owns only live width resizing.
//! Visibility / reorder can build on the same state later.

use iced::Task;

use super::super::columns::TrackColumn;
use super::super::state::{ActiveColumnResize, Message, Sonora};

const MIN_TRACK_COLUMN_WIDTH: f32 = 40.0;

fn find_column_mut(
    state: &mut Sonora,
    kind: TrackColumn,
) -> Option<&mut super::super::columns::TrackColumnState> {
    state.track_columns.iter_mut().find(|c| c.kind == kind)
}

pub(crate) fn start_track_column_resize(state: &mut Sonora, column: TrackColumn) -> Task<Message> {
    let Some(col) = state.track_columns.iter().find(|c| c.kind == column) else {
        return Task::none();
    };

    state.active_column_resize = Some(ActiveColumnResize {
        column,
        anchor_x: None,
        start_width: col.width.max(MIN_TRACK_COLUMN_WIDTH),
    });

    Task::none()
}

pub(crate) fn update_track_column_resize(state: &mut Sonora, cursor_x: f32) -> Task<Message> {
    let Some(active) = state.active_column_resize.as_mut() else {
        return Task::none();
    };

    let Some(anchor_x) = active.anchor_x else {
        active.anchor_x = Some(cursor_x);
        return Task::none();
    };

    let column = active.column;
    let new_width = (active.start_width + (cursor_x - anchor_x)).max(MIN_TRACK_COLUMN_WIDTH);

    if let Some(col) = find_column_mut(state, column) {
        col.width = new_width;
    }

    Task::none()
}

pub(crate) fn end_track_column_resize(state: &mut Sonora) -> Task<Message> {
    state.active_column_resize = None;
    Task::none()
}
