//! gui/view/mod.rs
//! GUI renderer (reads state, produces widgets; no mutation).

mod albums;
mod center;
pub(crate) mod constants;
mod inspector;
mod sidebar;
mod tracks;
mod widgets;

use iced::Length;
use iced::widget::{Column, column, row};

use super::state::{Message, Sonora};
use constants::{EDITOR_W, PLAYBACK_H, SIDEBAR_W};

pub(crate) fn view(state: &Sonora) -> Column<'_, Message> {
    let playback = widgets::playback_bar(state).height(Length::Fixed(PLAYBACK_H));

    let sidebar = sidebar::build_sidebar(state).width(Length::Fixed(SIDEBAR_W));
    let main = center::build_center_panel(state).width(Length::Fill);

    // Only show the inspector when something is selected
    let has_selection = state.selected_track.is_some() || !state.selected_tracks.is_empty();

    let body = if has_selection {
        let editor = inspector::build_inspector_panel(state).width(Length::Fixed(EDITOR_W));
        row![sidebar, main, editor].spacing(12).height(Length::Fill)
    } else {
        row![sidebar, main].spacing(12).height(Length::Fill)
    };

    column![playback, body].spacing(12).padding(12)
}
