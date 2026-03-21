//! gui/view/mod.rs
//! GUI renderer (reads state, produces widgets; no mutation).

use std::time::Instant;

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
use constants::{EDITOR_W, SIDEBAR_W};

pub(crate) fn view(state: &Sonora) -> Column<'_, Message> {
    let started = Instant::now();

    let playback_started = Instant::now();
    let playback = widgets::playback_bar(state).width(Length::Fill);
    let playback_ms = playback_started.elapsed().as_secs_f64() * 1000.0;

    let sidebar_started = Instant::now();
    let sidebar = sidebar::build_sidebar(state).width(Length::Fixed(SIDEBAR_W));
    let sidebar_ms = sidebar_started.elapsed().as_secs_f64() * 1000.0;

    let main_started = Instant::now();
    let main = center::build_center_panel(state).width(Length::Fill);
    let main_ms = main_started.elapsed().as_secs_f64() * 1000.0;

    let show_inspector = state.inspector_open && state.has_selection();

    let inspector_ms = if show_inspector {
        let inspector_started = Instant::now();
        let _ = inspector::build_inspector_panel(state).width(Length::Fixed(EDITOR_W));
        inspector_started.elapsed().as_secs_f64() * 1000.0
    } else {
        0.0
    };

    let body = if show_inspector {
        let editor = inspector::build_inspector_panel(state).width(Length::Fixed(EDITOR_W));
        row![sidebar, main, editor].spacing(12).height(Length::Fill)
    } else {
        row![sidebar, main].spacing(12).height(Length::Fill)
    };

    let total_ms = started.elapsed().as_secs_f64() * 1000.0;

    eprintln!(
        "[PERF][view::root] playback_ms={:.2} sidebar_ms={:.2} main_ms={:.2} inspector_ms={:.2} show_inspector={} total_ms={:.2}",
        playback_ms, sidebar_ms, main_ms, inspector_ms, show_inspector, total_ms
    );

    column![playback, body].spacing(12).padding(12)
}
