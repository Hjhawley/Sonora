//! gui/view/center.rs
//!
//! Center panel router (tracks vs albums).

use std::time::Instant;

use iced::widget::container;

use super::super::state::{Message, Sonora, ViewMode};
use super::albums::build_albums_center;
use super::tracks::build_tracks_center;

pub(crate) fn build_center_panel(state: &Sonora) -> iced::widget::Container<'_, Message> {
    let started = Instant::now();

    let inner: iced::Element<'_, Message> = match state.view_mode {
        ViewMode::Tracks => build_tracks_center(state).into(),
        ViewMode::Albums => build_albums_center(state).into(),
    };

    let total_ms = started.elapsed().as_secs_f64() * 1000.0;
    eprintln!(
        "[PERF][view::center] mode={:?} total_ms={:.2}",
        state.view_mode, total_ms
    );

    container(inner).padding(12)
}
