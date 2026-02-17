use iced::Task;
use std::path::{Path, PathBuf};

use super::super::state::Message;
use super::super::state::Sonora;

pub(crate) fn root_input_changed(state: &mut Sonora, s: String) -> Task<Message> {
    state.root_input = s;
    Task::none()
}

pub(crate) fn add_root_pressed(state: &mut Sonora) -> Task<Message> {
    if state.scanning || state.saving {
        return Task::none();
    }

    let input = state.root_input.trim();
    if input.is_empty() {
        return Task::none();
    }

    let p = PathBuf::from(input);

    // Validate: user must add an existing directory.
    if !Path::new(input).is_dir() {
        state.status = format!("Not a folder: {}", p.display());
        return Task::none();
    }

    // Avoid duplicates.
    if state.roots.contains(&p) {
        state.status = format!("Already added: {}", p.display());
        state.root_input.clear();
        return Task::none();
    }

    state.roots.push(p.clone());
    state.root_input.clear();
    state.status = format!("Added folder: {}", p.display());
    Task::none()
}

pub(crate) fn remove_root(state: &mut Sonora, i: usize) -> Task<Message> {
    if i < state.roots.len() && !state.scanning && !state.saving {
        let removed = state.roots.remove(i);
        state.status = format!("Removed folder: {}", removed.display());
    }
    Task::none()
}
