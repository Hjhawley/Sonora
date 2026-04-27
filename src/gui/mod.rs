//! gui/mod.rs
//!
//! Frontend concerns:
//! - app state ('Sonora')
//! - messages ('Message')
//! - update logic ('update()')
//! - view layout ('view()')
//! - subscriptions (polling playback events)
//! - small UI helpers ('util')

use iced::Task;

pub(crate) mod columns;
pub(crate) mod query;
pub(crate) mod state;
pub(crate) mod subscription;
pub(crate) mod theme;
pub(crate) mod update;
pub(crate) mod util;
pub(crate) mod view;

// Re-export the entry points main.rs needs.
pub(crate) use state::{Message, Sonora};
pub(crate) use subscription::subscription;
pub(crate) use update::update;
pub(crate) use view::view;

pub(crate) fn boot() -> (Sonora, Task<Message>) {
    let mut state = Sonora::default();

    let task = if state.view_mode == state::ViewMode::Albums {
        update::selection::preload_album_covers(&mut state)
    } else {
        Task::none()
    };

    (state, task)
}
