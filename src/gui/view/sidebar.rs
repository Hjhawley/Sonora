//! gui/view/sidebar.rs
//! Left sidebar (scan, scope toggles, layout toggles, action buttons, roots list).
//!
//! Phase-1 visual structure pass:
//! - stronger section rhythm
//! - clearer hierarchy
//! - more deliberate spacing
//! - more consistent control sizing
//!
//! No behavior changes here; this is purely layout / hierarchy cleanup.

use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Color, Length};

use super::super::state::{LibraryScope, Message, Sonora, ViewMode};
use super::style::{sonora_button, sonora_button_muted, sonora_button_selected, sonora_input};

/// Slightly muted utility text for low-priority status/readout lines.
const SECONDARY_TEXT: Color = Color::from_rgb8(0xB8, 0xB8, 0xB8);

fn section_title<'a>(label: &'a str) -> iced::widget::Text<'a> {
    text(label).size(14)
}

pub(crate) fn build_sidebar(state: &Sonora) -> iced::widget::Container<'_, Message> {
    let busy = state.scanning || state.saving;
    let has_selection = state.has_selection();

    // Top-level primary action for the sidebar.
    // Give it full width so it reads like a real section action,
    // not a loose floating button.
    let scan_btn = if state.scanning {
        button("Scanning...").style(sonora_button)
    } else {
        button("Scan Library")
            .on_press(Message::ScanLibrary)
            .style(sonora_button)
    }
    .width(Length::Fill);

    // Scope buttons: equal width so the section reads as one coherent control group.
    let library_btn = if state.library_scope == LibraryScope::Library {
        button("▷ Library").style(sonora_button_selected)
    } else {
        button("Library")
            .on_press(Message::SetLibraryScope(LibraryScope::Library))
            .style(sonora_button_muted)
    }
    .width(Length::FillPortion(1));

    let hidden_btn = if state.library_scope == LibraryScope::Hidden {
        button("▷ Hidden").style(sonora_button_selected)
    } else {
        button("Hidden")
            .on_press(Message::SetLibraryScope(LibraryScope::Hidden))
            .style(sonora_button_muted)
    }
    .width(Length::FillPortion(1));

    let missing_btn = if state.library_scope == LibraryScope::Missing {
        button("▷ Missing").style(sonora_button_selected)
    } else {
        button("Missing")
            .on_press(Message::SetLibraryScope(LibraryScope::Missing))
            .style(sonora_button_muted)
    }
    .width(Length::FillPortion(1));

    let scope_toggle = row![library_btn, hidden_btn, missing_btn].spacing(8);

    // Layout mode buttons: same sizing rule as scope toggles.
    let albums_btn = if state.view_mode == ViewMode::Albums {
        button("▷ Album View").style(sonora_button_selected)
    } else {
        button("Album View")
            .on_press(Message::SetViewMode(ViewMode::Albums))
            .style(sonora_button_muted)
    }
    .width(Length::FillPortion(1));

    let tracks_btn = if state.view_mode == ViewMode::Tracks {
        button("▷ Track View").style(sonora_button_selected)
    } else {
        button("Track View")
            .on_press(Message::SetViewMode(ViewMode::Tracks))
            .style(sonora_button_muted)
    }
    .width(Length::FillPortion(1));

    let view_toggle = row![albums_btn, tracks_btn].spacing(8);

    // Context-sensitive action section:
    // still one button for now, but giving it its own group makes the sidebar
    // read as navigation + layout + actions instead of one flat pile of controls.
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
    }
    .width(Length::Fill);

    let root_input = text_input("Add folder path", &state.root_input)
        .on_input(Message::RootInputChanged)
        .on_submit(Message::AddRootPressed)
        .width(Length::Fill)
        .style(sonora_input);

    // Keep Add compact but intentional.
    let add_btn = if busy {
        button("Add").style(sonora_button)
    } else {
        button("Add")
            .on_press(Message::AddRootPressed)
            .style(sonora_button)
    }
    .width(Length::Fixed(64.0));

    let add_row = row![root_input, add_btn].spacing(8);

    let mut roots_list = column![];

    for (i, p) in state.roots.iter().enumerate() {
        let remove_btn = if busy {
            button("✕").style(sonora_button)
        } else {
            button("✕")
                .on_press(Message::RemoveRoot(i))
                .style(sonora_button)
        }
        .width(Length::Fixed(40.0));

        // Keep roots visually compact, but give them enough breathing room that
        // the list feels maintained rather than dumped into the panel.
        let path_txt = text(p.display().to_string())
            .size(12)
            .color(SECONDARY_TEXT)
            .width(Length::Fill);

        roots_list = roots_list.push(
            row![path_txt, remove_btn]
                .spacing(8)
                .align_y(iced::Alignment::Center),
        );
    }

    let roots_panel = scrollable(roots_list.spacing(8)).height(Length::Fixed(180.0));

    let scope_label = match state.library_scope {
        LibraryScope::Library => "Library",
        LibraryScope::Hidden => "Hidden",
        LibraryScope::Missing => "Missing",
    };

    // Sidebar hierarchy:
    // - top status/scan cluster
    // - clearly separated sections
    // - more vertical rhythm between sections than within sections
    let col = column![
        // Status cluster:
        // visually low priority, but kept near Scan because they are part of the
        // same mental model: "what state is the library in right now?"
        column![text(&state.status).size(12).color(SECONDARY_TEXT), scan_btn,].spacing(10),
        // Scope section
        column![
            section_title("Scope"),
            scope_toggle,
            text(format!("Current: {scope_label}"))
                .size(12)
                .color(SECONDARY_TEXT),
        ]
        .spacing(8),
        // Layout section
        column![section_title("Layout"), view_toggle,].spacing(8),
        // Actions section
        column![section_title("Actions"), visibility_btn,].spacing(8),
        // Folder management section
        column![
            section_title("Library folders"),
            text("Saved library roots").size(12).color(SECONDARY_TEXT),
            add_row,
            roots_panel,
        ]
        .spacing(8),
    ]
    // Larger gap between sections than inside sections.
    // This is the main rhythm fix for the sidebar.
    .spacing(18);

    container(scrollable(col).height(Length::Fill)).padding(14)
}
