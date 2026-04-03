//! gui/view/sidebar.rs
//! Left sidebar (scan, scope toggles, layout toggles, roots list).

use iced::Length;
use iced::widget::{button, column, container, row, scrollable, text, text_input};

use super::super::state::{LibraryScope, Message, Sonora, ViewMode};
use super::style::{sonora_button, sonora_button_muted, sonora_button_selected, sonora_input};

pub(crate) fn build_sidebar(state: &Sonora) -> iced::widget::Container<'_, Message> {
    let busy = state.scanning || state.saving;
    let has_selection = state.has_selection();

    let scan_btn = if state.scanning {
        button("Scanning...").style(sonora_button)
    } else {
        button("Scan Library")
            .on_press(Message::ScanLibrary)
            .style(sonora_button)
    };

    let library_btn = if state.library_scope == LibraryScope::Library {
        button("▷ Library").style(sonora_button_selected)
    } else {
        button("Library")
            .on_press(Message::SetLibraryScope(LibraryScope::Library))
            .style(sonora_button_muted)
    };

    let hidden_btn = if state.library_scope == LibraryScope::Hidden {
        button("▷ Hidden").style(sonora_button_selected)
    } else {
        button("Hidden")
            .on_press(Message::SetLibraryScope(LibraryScope::Hidden))
            .style(sonora_button_muted)
    };

    let missing_btn = if state.library_scope == LibraryScope::Missing {
        button("▷ Missing").style(sonora_button_selected)
    } else {
        button("Missing")
            .on_press(Message::SetLibraryScope(LibraryScope::Missing))
            .style(sonora_button_muted)
    };

    let scope_toggle = row![library_btn, hidden_btn, missing_btn].spacing(8);

    let albums_btn = if state.view_mode == ViewMode::Albums {
        button("▷ Album View").style(sonora_button_selected)
    } else {
        button("Album View")
            .on_press(Message::SetViewMode(ViewMode::Albums))
            .style(sonora_button_muted)
    };

    let tracks_btn = if state.view_mode == ViewMode::Tracks {
        button("▷ Track View").style(sonora_button_selected)
    } else {
        button("Track View")
            .on_press(Message::SetViewMode(ViewMode::Tracks))
            .style(sonora_button_muted)
    };

    let view_toggle = row![albums_btn, tracks_btn].spacing(8);

    let visibility_btn = match state.library_scope {
        LibraryScope::Library => {
            if busy || !has_selection {
                button("Hide from Sonora").style(sonora_button_muted)
            } else {
                button("Hide from Sonora")
                    .on_press(Message::HideSelected)
                    .style(sonora_button_muted)
            }
        }
        LibraryScope::Hidden => {
            if busy || !has_selection {
                button("Unhide").style(sonora_button_muted)
            } else {
                button("Unhide")
                    .on_press(Message::UnhideSelected)
                    .style(sonora_button_muted)
            }
        }
        LibraryScope::Missing => {
            if busy || !has_selection {
                button("Delete from Sonora").style(sonora_button_muted)
            } else {
                button("Delete from Sonora")
                    .on_press(Message::DeleteSelectedFromSonora)
                    .style(sonora_button_muted)
            }
        }
    };

    let root_input = text_input("Add folder path", &state.root_input)
        .on_input(Message::RootInputChanged)
        .on_submit(Message::AddRootPressed)
        .width(Length::Fill)
        .style(sonora_input);

    let add_btn = if busy {
        button("Add").style(sonora_button)
    } else {
        button("Add")
            .on_press(Message::AddRootPressed)
            .style(sonora_button)
    };

    let add_row = row![root_input, add_btn].spacing(8);

    let mut roots_list = column![];
    for (i, p) in state.roots.iter().enumerate() {
        let remove_btn = if busy {
            button("✕").style(sonora_button)
        } else {
            button("✕")
                .on_press(Message::RemoveRoot(i))
                .style(sonora_button)
        };

        let path_txt = text(p.display().to_string()).size(12).width(Length::Fill);

        roots_list = roots_list.push(
            row![path_txt, remove_btn]
                .spacing(8)
                .align_y(iced::Alignment::Center),
        );
    }
    let roots_panel = scrollable(roots_list.spacing(6)).height(Length::Fixed(160.0));

    let scope_label = match state.library_scope {
        LibraryScope::Library => "Library",
        LibraryScope::Hidden => "Hidden",
        LibraryScope::Missing => "Missing",
    };

    // sidebar layout
    let col = column![
        text(&state.status).size(12),
        scan_btn,
        text("Scope").size(16),
        scope_toggle,
        text("Layout").size(16),
        view_toggle,
        text("Actions").size(16),
        visibility_btn,
        text("Library folders").size(16),
        add_row,
        roots_panel,
        text(format!("Current view: {scope_label}")).size(12),
    ]
    .spacing(12);

    container(scrollable(col).height(Length::Fill)).padding(12)
}
