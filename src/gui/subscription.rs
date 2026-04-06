//! gui/subscription.rs
//! Global subscriptions:
//! - keyboard event listener
//! - playback polling while active
//! - mouse movement / release for Track View column resizing

use iced::{Subscription, event, time};
use std::time::Duration;

use super::state::{Message, Sonora};

fn map_event(
    event: iced::Event,
    _status: iced::event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    match event {
        iced::Event::Keyboard(key_event) => Some(Message::KeyboardEvent(key_event)),

        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            Some(Message::UpdateTrackColumnResize {
                cursor_x: position.x,
            })
        }

        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
            Some(Message::EndTrackColumnResize)
        }

        _ => None,
    }
}

pub(crate) fn subscription(state: &Sonora) -> Subscription<Message> {
    let event_sub = event::listen_with(map_event);

    let should_poll = state.playback_events.is_some()
        && (state.is_playing
            || state.awaiting_started
            || state.seek_preview_ratio.is_some()
            || state.now_playing.is_some());

    if should_poll {
        Subscription::batch(vec![
            event_sub,
            time::every(Duration::from_millis(200)).map(|_| Message::TickPlayback),
        ])
    } else {
        Subscription::batch(vec![event_sub])
    }
}
