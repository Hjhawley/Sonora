//! Left sidebar (scan, view toggles, roots list, playlists).

use iced::Length;
use iced::widget::{button, column, container, row, scrollable, text, text_input};

use super::super::state::{Message, Sonora, ViewMode};

pub(crate) fn build_sidebar(state: &Sonora) -> iced::widget::Container<'_, Message> {
    let scan_btn = if state.scanning {
        button("Scanning...")
    } else {
        button("Scan Library").on_press(Message::ScanLibrary)
    };

    let albums_btn = if state.view_mode == ViewMode::Albums {
        button("✓ Album View")
    } else {
        button("Album View").on_press(Message::SetViewMode(ViewMode::Albums))
    };

    let tracks_btn = if state.view_mode == ViewMode::Tracks {
        button("✓ Track View")
    } else {
        button("Track View").on_press(Message::SetViewMode(ViewMode::Tracks))
    };

    let view_toggle = row![albums_btn, tracks_btn].spacing(8);

    let root_input = text_input("Add folder path", &state.root_input)
        .on_input(Message::RootInputChanged)
        .on_submit(Message::AddRootPressed)
        .width(Length::Fill);

    let add_btn = if state.scanning {
        button("Add")
    } else {
        button("Add").on_press(Message::AddRootPressed)
    };

    let add_row = row![root_input, add_btn].spacing(8);

    let mut roots_list = column![];
    for (i, p) in state.roots.iter().enumerate() {
        let remove_btn = if state.scanning {
            button("×")
        } else {
            button("×").on_press(Message::RemoveRoot(i))
        };

        roots_list = roots_list.push(row![text(p.display().to_string()), remove_btn].spacing(8));
    }
    let roots_panel = scrollable(roots_list.spacing(6)).height(Length::Fixed(160.0));

    let playlists = column![
        text("Playlists").size(16),
        button("Library"),
        button("Favorites (coming soon)"),
        button("Recently added (coming soon)"),
    ]
    .spacing(6);

    let col = column![
        text("Sonora").size(20),
        text(&state.status).size(12),
        scan_btn,
        view_toggle,
        text("Library folders").size(16),
        add_row,
        roots_panel,
        playlists,
    ]
    .spacing(12);

    container(scrollable(col).height(Length::Fill)).padding(12)
}
