//! gui/subscription.rs
//! Global subscriptions:
//! - keyboard event listener
//! - playback polling while active

use iced::{Subscription, event, time};
use std::time::Duration;

use super::state::{Message, Sonora};

pub(crate) fn subscription(state: &Sonora) -> Subscription<Message> {
    let keyboard_sub = event::listen_with(|event, _status, _window| match event {
        iced::Event::Keyboard(key_event) => Some(Message::KeyboardEvent(key_event)),
        _ => None,
    });

    let should_poll = state.playback_events.is_some()
        && (state.is_playing
            || state.awaiting_started
            || state.seek_preview_ratio.is_some()
            || state.now_playing.is_some());

    if should_poll {
        Subscription::batch(vec![
            keyboard_sub,
            time::every(Duration::from_millis(200)).map(|_| Message::TickPlayback),
        ])
    } else {
        Subscription::batch(vec![keyboard_sub])
    }
}
