//! gui/update/playback.rs
//! bridge between GUI and playback engine

use iced::Task;

use super::super::state::{Message, Sonora};
use crate::core::playback::{PlayerCommand, PlayerEvent, start_playback};

fn ensure_engine(state: &mut Sonora) {
    if state.playback.is_some() && state.playback_events.is_some() {
        return;
    }

    let (controller, events) = start_playback();
    controller.send(PlayerCommand::SetVolume(state.volume));

    state.playback = Some(controller);
    state.playback_events = Some(std::cell::RefCell::new(events));
}

pub(crate) fn drain_events(state: &mut Sonora) -> Task<Message> {
    let Some(rx_cell) = state.playback_events.as_ref() else {
        return Task::none();
    };

    // 1) Drain into a local vec while holding ONLY the receiver borrow.
    let mut drained: Vec<PlayerEvent> = Vec::new();
    {
        let rx = rx_cell.borrow_mut();
        while let Ok(ev) = rx.try_recv() {
            drained.push(ev);
        }
    } // receiver borrow dropped here

    // 2) Now freely mutate state.
    for ev in drained {
        let _ = handle_event(state, ev);
    }

    Task::none()
}

pub(crate) fn play_selected(state: &mut Sonora) -> Task<Message> {
    let Some(i) = state.selected_track else {
        state.status = "No track selected.".into();
        return Task::none();
    };
    play_track(state, i)
}

pub(crate) fn play_track(state: &mut Sonora, index: usize) -> Task<Message> {
    ensure_engine(state);

    let Some(controller) = &state.playback else {
        state.status = "Playback engine failed to initialize.".into();
        return Task::none();
    };

    let Some(row) = state.tracks.get(index) else {
        state.status = "Play failed: track index out of range.".into();
        return Task::none();
    };

    let path = row.path.clone();

    controller.send(PlayerCommand::PlayFile(path.clone()));

    // Playback should not hijack selection.
    state.now_playing = Some(index);
    state.is_playing = true;
    state.position_ms = 0;
    state.duration_ms = None;
    state.status = format!("Playing: {}", path.display());

    Task::none()
}

pub(crate) fn toggle_play_pause(state: &mut Sonora) -> Task<Message> {
    if state.is_playing {
        return pause(state);
    }

    if state.now_playing.is_some() {
        resume(state)
    } else {
        play_selected(state)
    }
}

pub(crate) fn pause(state: &mut Sonora) -> Task<Message> {
    ensure_engine(state);

    let Some(controller) = &state.playback else {
        state.status = "Pause failed: playback engine failed to initialize.".into();
        return Task::none();
    };

    controller.send(PlayerCommand::Pause);
    state.is_playing = false;

    Task::none()
}

pub(crate) fn resume(state: &mut Sonora) -> Task<Message> {
    if state.now_playing.is_none() {
        return play_selected(state);
    }

    ensure_engine(state);

    let Some(controller) = &state.playback else {
        state.status = "Resume failed: playback engine failed to initialize.".into();
        return Task::none();
    };

    controller.send(PlayerCommand::Resume);
    state.is_playing = true;

    Task::none()
}

pub(crate) fn stop(state: &mut Sonora) -> Task<Message> {
    ensure_engine(state);

    let Some(controller) = &state.playback else {
        state.status = "Stop failed: playback engine failed to initialize.".into();
        return Task::none();
    };

    controller.send(PlayerCommand::Stop);

    state.is_playing = false;
    state.position_ms = 0;
    state.duration_ms = None;

    Task::none()
}

pub(crate) fn next(state: &mut Sonora) -> Task<Message> {
    if state.tracks.is_empty() {
        return Task::none();
    }

    // Drive playback from now_playing; fall back to selection; else 0.
    let cur = state.now_playing.or(state.selected_track).unwrap_or(0);

    // Wrap at end
    let next = if cur + 1 >= state.tracks.len() {
        0
    } else {
        cur + 1
    };

    play_track(state, next)
}

pub(crate) fn prev(state: &mut Sonora) -> Task<Message> {
    if state.tracks.is_empty() {
        return Task::none();
    }

    let cur = state.now_playing.or(state.selected_track).unwrap_or(0);

    // Wrap at beginning
    let prev = if cur == 0 {
        state.tracks.len() - 1
    } else {
        cur - 1
    };

    play_track(state, prev)
}

pub(crate) fn seek(state: &mut Sonora, ratio: f32) -> Task<Message> {
    let Some(dur_ms) = state.duration_ms else {
        return Task::none();
    };

    ensure_engine(state);

    let Some(controller) = &state.playback else {
        return Task::none();
    };

    let ratio = ratio.clamp(0.0, 1.0);
    let target_ms = ((ratio as f64) * (dur_ms as f64)).round() as u64;

    controller.send(PlayerCommand::Seek(target_ms));
    // Optimistic UI update; engine will confirm via Started/Position.
    state.position_ms = target_ms.min(dur_ms);

    Task::none()
}

pub(crate) fn set_volume(state: &mut Sonora, volume: f32) -> Task<Message> {
    let volume = volume.clamp(0.0, 1.0);
    state.volume = volume;

    if let Some(controller) = &state.playback {
        controller.send(PlayerCommand::SetVolume(volume));
    }

    Task::none()
}

pub(crate) fn handle_event(state: &mut Sonora, event: PlayerEvent) -> Task<Message> {
    match event {
        PlayerEvent::Started {
            path,
            duration_ms,
            start_ms,
        } => {
            state.is_playing = true;
            state.duration_ms = duration_ms;
            state.position_ms = start_ms; // <-- don't smash seeks back to 0
            state.status = format!("Now playing: {}", path.display());
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
        }
    }

    Task::none()
}
