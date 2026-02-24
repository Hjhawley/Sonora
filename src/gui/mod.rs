//! gui/mod.rs
//!
//! This folder contains ONLY frontend concerns:
//! - app state ('Sonora')
//! - messages ('Message')
//! - update logic ('update()')
//! - view layout ('view()')
//! - subscriptions (polling playback events)
//! - small UI helpers ('util')

pub(crate) mod state;
pub(crate) mod subscription;
pub(crate) mod update;
pub(crate) mod util;
pub(crate) mod view;

// Re-export the entry points main.rs needs.
pub(crate) use state::Sonora;
pub(crate) use subscription::subscription;
pub(crate) use update::update;
pub(crate) use view::view;
