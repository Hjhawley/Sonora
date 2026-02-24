//! gui/subscription.rs
//! Poll playback events by emitting a periodic TickPlayback message.

use iced::{Subscription, time};
use std::time::Duration;

use super::state::{Message, Sonora};

pub(crate) fn subscription(state: &Sonora) -> Subscription<Message> {
    if state.playback_events.is_none() {
        return Subscription::none();
    }

    time::every(Duration::from_millis(200)).map(|_| Message::TickPlayback)
}
