//! gui/subscription.rs
//! Poll playback events and listen for global keyboard events.

use iced::{Subscription, event, time};
use std::time::Duration;

use super::state::{Message, Sonora};

pub(crate) fn subscription(state: &Sonora) -> Subscription<Message> {
    let tick = if state.playback_events.is_some() {
        time::every(Duration::from_millis(200)).map(|_| Message::TickPlayback)
    } else {
        Subscription::none()
    };

    let keyboard = event::listen().map(|event| match event {
        iced::Event::Keyboard(key_event) => Message::KeyboardEvent(key_event),
        _ => Message::Noop,
    });

    Subscription::batch(vec![tick, keyboard])
}
