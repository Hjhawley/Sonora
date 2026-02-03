//! GUI module (Iced).
//!
//! This folder contains ONLY UI concerns:
//! - app state (`Sonora`)
//! - messages (`Message`)
//! - update logic (`update()`)
//! - view layout (`view()`)
//! - small UI helpers (`util`)
//!
//! The filesystem scanning + tag reading lives in `crate::core`.

pub(crate) mod state;
pub(crate) mod update;
pub(crate) mod util;
pub(crate) mod view;

// Re-export the entry points main.rs needs.
pub(crate) use state::Sonora;
pub(crate) use update::update;
pub(crate) use view::view;
