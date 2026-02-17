//! GUI renderer (reads state, produces widgets; no mutation).

mod albums;
mod center;
mod constants;
mod inspector;
mod sidebar;
mod tracks;
mod widgets;

use iced::Length;
use iced::widget::{Column, column, row};

use super::state::{Message, Sonora};
use constants::{EDITOR_W, PLAYBACK_H, SIDEBAR_W};

pub(crate) fn view(state: &Sonora) -> Column<'_, Message> {
    let playback = widgets::playback_bar().height(Length::Fixed(PLAYBACK_H));

    let sidebar = sidebar::build_sidebar(state).width(Length::Fixed(SIDEBAR_W));
    let main = center::build_center_panel(state).width(Length::Fill);
    let editor = inspector::build_inspector_panel(state).width(Length::Fixed(EDITOR_W));

    let body = row![sidebar, main, editor].spacing(12).height(Length::Fill);
    column![playback, body].spacing(12).padding(12)
}
