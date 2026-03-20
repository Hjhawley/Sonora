//! gui/subscription.rs
//! Poll playback events only when playback activity exists.

use iced::{Subscription, time};
use std::time::Duration;

use super::state::{Message, Sonora};

pub(crate) fn subscription(state: &Sonora) -> Subscription<Message> {
    let should_poll = state.playback_events.is_some()
        && (state.is_playing
            || state.awaiting_started
            || state.seek_preview_ratio.is_some()
            || state.now_playing.is_some());

    if should_poll {
        time::every(Duration::from_millis(200)).map(|_| Message::TickPlayback)
    } else {
        Subscription::none()
    }
}
