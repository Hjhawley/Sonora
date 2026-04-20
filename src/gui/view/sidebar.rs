//! gui/view/sidebar.rs
//! Left sidebar (scan, scope toggles, layout toggles, action buttons, roots list).

use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Length};

use super::super::state::{LibraryScope, Message, Sonora, ViewMode};
use super::constants::{PANEL_PAD, PANEL_SUBGROUP_SPACING};
use super::shared::{content_group, helper_text, panel_stack, section_block};
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
    }
    .width(Length::Fill);

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

    let add_btn = if busy {
        button("Add").style(sonora_button)
    } else {
        button("Add")
            .on_press(Message::AddRootPressed)
            .style(sonora_button)
    }
    .width(Length::Fixed(64.0));

    let add_row = row![root_input, add_btn]
        .spacing(8)
        .align_y(Alignment::Center);

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

        let path_txt = text(p.display().to_string()).size(12).width(Length::Fill);

        roots_list = roots_list.push(
            row![path_txt, remove_btn]
                .spacing(8)
                .align_y(Alignment::Center),
        );
    }

    let roots_panel =
        scrollable(roots_list.spacing(PANEL_SUBGROUP_SPACING)).height(Length::Fixed(180.0));

    let scope_label = match state.library_scope {
        LibraryScope::Library => "Library",
        LibraryScope::Hidden => "Hidden",
        LibraryScope::Missing => "Missing",
    };

    let status_block = content_group([helper_text(state.status.clone()).into(), scan_btn.into()]);

    let scope_block = section_block(
        "Scope",
        content_group([
            scope_toggle.into(),
            helper_text(format!("Current: {scope_label}")).into(),
        ]),
    );

    let layout_block = section_block("Layout", view_toggle);

    let actions_block = section_block("Actions", visibility_btn);

    let folders_block = section_block(
        "Library folders",
        content_group([
            helper_text("Saved library roots").into(),
            add_row.into(),
            roots_panel.into(),
        ]),
    );

    let content = panel_stack([
        status_block.into(),
        scope_block.into(),
        layout_block.into(),
        actions_block.into(),
        folders_block.into(),
    ]);

    container(scrollable(content).height(Length::Fill)).padding(PANEL_PAD)
}
