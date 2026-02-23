//! GUI ↔ playback engine bridge.
//!
//! Translate GUI actions -> PlayerCommand
//! Translate PlayerEvent -> GUI state updates
//!
//! No rodio usage here.

use iced::Task;

use super::super::state::{Message, Sonora};
use crate::core::playback::{PlayerCommand, PlayerEvent};

pub(crate) fn play_selected(state: &mut Sonora) -> Task<Message> {
    let Some(i) = state.selected_track else {
        state.status = "No track selected.".into();
        return Task::none();
    };
    play_track(state, i)
}

pub(crate) fn play_track(state: &mut Sonora, index: usize) -> Task<Message> {
    let Some(controller) = &state.playback else {
        state.status = "Playback engine not initialized".into();
        return Task::none();
    };

    if index >= state.tracks.len() {
        return Task::none();
    }

    let path = state.tracks[index].path.clone();
    controller.send(PlayerCommand::PlayFile(path));

    state.now_playing = Some(index);
    state.is_playing = true;
    state.position_ms = 0;
    state.duration_ms = None;

    Task::none()
}

pub(crate) fn toggle_play_pause(state: &mut Sonora) -> Task<Message> {
    if state.is_playing {
        pause(state)
    } else {
        resume(state)
    }
}

pub(crate) fn pause(state: &mut Sonora) -> Task<Message> {
    let Some(controller) = &state.playback else {
        return Task::none();
    };

    controller.send(PlayerCommand::Pause);
    state.is_playing = false;

    Task::none()
}

pub(crate) fn resume(state: &mut Sonora) -> Task<Message> {
    let Some(controller) = &state.playback else {
        return Task::none();
    };

    controller.send(PlayerCommand::Resume);
    state.is_playing = true;

    Task::none()
}

pub(crate) fn stop(state: &mut Sonora) -> Task<Message> {
    let Some(controller) = &state.playback else {
        return Task::none();
    };

    controller.send(PlayerCommand::Stop);

    state.is_playing = false;
    state.position_ms = 0;
    state.duration_ms = None;

    Task::none()
}

pub(crate) fn next(_state: &mut Sonora) -> Task<Message> {
    // Queue not implemented yet.
    Task::none()
}

pub(crate) fn prev(_state: &mut Sonora) -> Task<Message> {
    // Queue not implemented yet.
    Task::none()
}

pub(crate) fn seek(state: &mut Sonora, ratio: f32) -> Task<Message> {
    let Some(controller) = &state.playback else {
        return Task::none();
    };

    let ratio = ratio.clamp(0.0, 1.0);

    let Some(dur_ms) = state.duration_ms else {
        // Can't seek without a known duration.
        return Task::none();
    };

    let target_ms = (ratio as f64 * dur_ms as f64).round() as u64;

    controller.send(PlayerCommand::Seek(target_ms));
    Task::none()
}

pub(crate) fn set_volume(state: &mut Sonora, volume: f32) -> Task<Message> {
    let Some(controller) = &state.playback else {
        return Task::none();
    };

    let volume = volume.clamp(0.0, 1.0);
    controller.send(PlayerCommand::SetVolume(volume));
    state.volume = volume;

    Task::none()
}

pub(crate) fn handle_event(state: &mut Sonora, event: PlayerEvent) -> Task<Message> {
    match event {
        PlayerEvent::Started {
            path: _,
            duration_ms,
        } => {
            state.is_playing = true;
            state.duration_ms = duration_ms;
            state.position_ms = 0;
        }
        PlayerEvent::Paused => state.is_playing = false,
        PlayerEvent::Resumed => state.is_playing = true,
        PlayerEvent::Stopped => {
            state.is_playing = false;
            state.position_ms = 0;
            state.duration_ms = None;
        }
        PlayerEvent::Position { position_ms } => state.position_ms = position_ms,
        PlayerEvent::TrackEnded => {
            state.is_playing = false;
            state.position_ms = 0;
        }
        PlayerEvent::Error(err) => {
            state.status = format!("Playback error: {err}");
            state.is_playing = false;
        }
    }

    Task::none()
}
