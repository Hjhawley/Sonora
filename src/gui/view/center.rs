//! Center panel router (tracks vs albums).

use iced::widget::container;

use super::super::state::{Message, Sonora, ViewMode};
use super::albums::build_albums_center;
use super::tracks::build_tracks_center;

pub(crate) fn build_center_panel(state: &Sonora) -> iced::widget::Container<'_, Message> {
    let inner: iced::Element<'_, Message> = match state.view_mode {
        ViewMode::Tracks => build_tracks_center(state).into(),
        ViewMode::Albums => build_albums_center(state).into(),
    };

    container(inner).padding(12)
}
