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
        state.status = "Playback engine not initialized (state.playback is None).".into();
        return Task::none();
    };

    let Some(row) = state.tracks.get(index) else {
        state.status = "Play failed: track index out of range.".into();
        return Task::none();
    };

    let path = row.path.clone();

    // Send command to engine
    controller.send(PlayerCommand::PlayFile(path.clone()));

    // Optimistic UI updates (engine will also confirm via Started/Error)
    state.now_playing = Some(index);
    state.is_playing = true;
    state.position_ms = 0;
    state.duration_ms = None;
    state.status = format!("Playing: {}", path.display());

    Task::none()
}

pub(crate) fn toggle_play_pause(state: &mut Sonora) -> Task<Message> {
    // If we're actively playing, pause.
    if state.is_playing {
        return pause(state);
    }

    // Not playing:
    // - If we have a loaded "now playing" track, resume.
    // - Otherwise, start playing the currently selected track.
    if state.now_playing.is_some() {
        resume(state)
    } else {
        play_selected(state)
    }
}

pub(crate) fn pause(state: &mut Sonora) -> Task<Message> {
    let Some(controller) = &state.playback else {
        state.status = "Pause failed: playback engine not initialized.".into();
        return Task::none();
    };

    controller.send(PlayerCommand::Pause);
    state.is_playing = false;

    Task::none()
}

pub(crate) fn resume(state: &mut Sonora) -> Task<Message> {
    let Some(controller) = &state.playback else {
        state.status = "Resume failed: playback engine not initialized.".into();
        return Task::none();
    };

    // If nothing has ever been started, Resume will do nothing.
    // In that case, behave like "Play Selected".
    if state.now_playing.is_none() {
        return play_selected(state);
    }

    controller.send(PlayerCommand::Resume);
    state.is_playing = true;

    Task::none()
}

pub(crate) fn stop(state: &mut Sonora) -> Task<Message> {
    let Some(controller) = &state.playback else {
        state.status = "Stop failed: playback engine not initialized.".into();
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

/// Seek slider sends a ratio 0.0..=1.0 (see widgets.rs).
pub(crate) fn seek(state: &mut Sonora, ratio: f32) -> Task<Message> {
    let Some(controller) = &state.playback else {
        return Task::none();
    };

    let Some(dur_ms) = state.duration_ms else {
        // Can't seek until we know duration
        return Task::none();
    };

    let ratio = ratio.clamp(0.0, 1.0);
    let target_ms = ((ratio as f64) * (dur_ms as f64)).round() as u64;

    controller.send(PlayerCommand::Seek(target_ms));
    // Optional optimistic update (engine will correct via Position ticks)
    state.position_ms = target_ms.min(dur_ms);

    Task::none()
}

pub(crate) fn set_volume(state: &mut Sonora, volume: f32) -> Task<Message> {
    let volume = volume.clamp(0.0, 1.0);
    state.volume = volume;

    if let Some(controller) = &state.playback {
        controller.send(PlayerCommand::SetVolume(volume));
    } else {
        // Still let the slider move even if engine isn't up.
        state.status = "Volume set (engine not initialized yet).".into();
    }

    Task::none()
}

pub(crate) fn handle_event(state: &mut Sonora, event: PlayerEvent) -> Task<Message> {
    match event {
        PlayerEvent::Started { path, duration_ms } => {
            state.is_playing = true;
            state.duration_ms = duration_ms;
            state.position_ms = 0;
            state.status = format!("Now playing: {}", path.display());
        }

        PlayerEvent::Paused => {
            state.is_playing = false;
        }

        PlayerEvent::Resumed => {
            state.is_playing = true;
        }

        PlayerEvent::Stopped => {
            state.is_playing = false;
            state.position_ms = 0;
            state.duration_ms = None;
        }

        PlayerEvent::Position { position_ms } => {
            state.position_ms = position_ms;
        }

        PlayerEvent::TrackEnded => {
            state.is_playing = false;
            state.position_ms = 0;
            // keep duration_ms; it's still useful for UI until next track
        }

        PlayerEvent::Error(err) => {
            state.status = format!("Playback error: {err}");
            state.is_playing = false;
        }
    }

    Task::none()
}
