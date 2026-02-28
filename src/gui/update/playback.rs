//! gui/update/playback.rs
//! GUI-playback engine bridge
//!
//! - `now_playing` and selection are `TrackId`, not Vec indices.
//! - `PlayTrack` accepts a `TrackId` and looks up the current row by id.
//!
//! Design goals:
//! - GUI never touches rodio/symphonia directly.
//! - All IO / timing is driven by the engine + TickPlayback polling.

use iced::Task;

use super::super::state::{Message, Sonora};
use crate::core::playback::{PlayerCommand, PlayerEvent, start_playback};
use crate::core::types::TrackId;

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

    let mut drained: Vec<PlayerEvent> = Vec::new();
    {
        // Receiver::try_recv only needs &self, so borrow() is enough.
        let rx = rx_cell.borrow();
        while let Ok(ev) = rx.try_recv() {
            drained.push(ev);
        }
    }

    for ev in drained {
        let _ = handle_event(state, ev);
    }

    Task::none()
}

pub(crate) fn play_selected(state: &mut Sonora) -> Task<Message> {
    let Some(id) = state.selected_track else {
        state.status = "No track selected.".into();
        return Task::none();
    };
    play_track(state, id)
}

pub(crate) fn play_track(state: &mut Sonora, id: TrackId) -> Task<Message> {
    ensure_engine(state);

    let Some(controller) = &state.playback else {
        state.status = "Playback engine failed to initialize.".into();
        return Task::none();
    };

    let Some(row) = state.track_by_id(id) else {
        state.status = "Play failed: selected track not found (rescan?).".into();
        return Task::none();
    };

    let path = row.path.clone();

    #[cfg(debug_assertions)]
    eprintln!("[GUI] PlayTrack id={} path={}", id, path.display());

    controller.send(PlayerCommand::PlayFile(path.clone()));

    // Playback should not hijack selection.
    state.now_playing = Some(id);
    state.is_playing = true;
    state.position_ms = 0;
    state.duration_ms = None;
    state.seek_preview_ratio = None;
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
    state.seek_preview_ratio = None;

    Task::none()
}

pub(crate) fn next(state: &mut Sonora) -> Task<Message> {
    if state.tracks.is_empty() {
        return Task::none();
    }

    // Prefer "now playing", else selection, else first track.
    let anchor_id = state
        .now_playing
        .or(state.selected_track)
        .or_else(|| state.tracks.first().and_then(|t| t.id));

    let Some(cur_id) = anchor_id else {
        state.status = "No playable track found (missing ids?).".into();
        return Task::none();
    };

    // Convert current id to index, then move by index in the current display order.
    let cur_idx = state.index_of_id(cur_id).unwrap_or(0);
    let next_idx = if cur_idx + 1 >= state.tracks.len() {
        0
    } else {
        cur_idx + 1
    };

    let Some(next_id) = state.tracks.get(next_idx).and_then(|t| t.id) else {
        state.status = "Next failed: track missing id.".into();
        return Task::none();
    };

    play_track(state, next_id)
}

pub(crate) fn prev(state: &mut Sonora) -> Task<Message> {
    if state.tracks.is_empty() {
        return Task::none();
    }

    let anchor_id = state
        .now_playing
        .or(state.selected_track)
        .or_else(|| state.tracks.first().and_then(|t| t.id));

    let Some(cur_id) = anchor_id else {
        state.status = "No playable track found (missing ids?).".into();
        return Task::none();
    };

    let cur_idx = state.index_of_id(cur_id).unwrap_or(0);
    let prev_idx = if cur_idx == 0 {
        state.tracks.len() - 1
    } else {
        cur_idx - 1
    };

    let Some(prev_id) = state.tracks.get(prev_idx).and_then(|t| t.id) else {
        state.status = "Prev failed: track missing id.".into();
        return Task::none();
    };

    play_track(state, prev_id)
}

/// Seek slider changed: preview only (UI updates, no engine command).
pub(crate) fn seek_preview(state: &mut Sonora, ratio: f32) -> Task<Message> {
    let Some(dur_ms) = state.duration_ms else {
        return Task::none();
    };

    let ratio = ratio.clamp(0.0, 1.0);
    state.seek_preview_ratio = Some(ratio);

    let target_ms = ((ratio as f64) * (dur_ms as f64)).round() as u64;
    state.position_ms = target_ms.min(dur_ms);

    #[cfg(debug_assertions)]
    eprintln!(
        "[GUI] SeekPreview ratio={} dur_ms={} => preview_ms={}",
        ratio, dur_ms, state.position_ms
    );

    Task::none()
}

/// Seek slider released: commit the last preview to the engine.
pub(crate) fn seek_commit(state: &mut Sonora) -> Task<Message> {
    let Some(dur_ms) = state.duration_ms else {
        state.seek_preview_ratio = None;
        return Task::none();
    };

    let Some(ratio) = state.seek_preview_ratio.take() else {
        return Task::none();
    };

    ensure_engine(state);

    let Some(controller) = &state.playback else {
        return Task::none();
    };

    let mut target_ms = ((ratio as f64) * (dur_ms as f64)).round() as u64;

    // Seeking to *exactly* the end tends to produce EOF weirdness; clamp slightly.
    if target_ms >= dur_ms {
        target_ms = dur_ms.saturating_sub(1);
    }

    #[cfg(debug_assertions)]
    eprintln!(
        "[GUI] SeekCommit ratio={} dur_ms={} => target_ms={}",
        ratio, dur_ms, target_ms
    );

    controller.send(PlayerCommand::Seek(target_ms));

    // Optimistic UI update; engine will confirm via Started/Position.
    state.position_ms = target_ms;

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
    #[cfg(debug_assertions)]
    match &event {
        PlayerEvent::Started {
            path,
            duration_ms,
            start_ms,
        } => {
            eprintln!(
                "[GUI] Event Started path={} duration_ms={:?} start_ms={}",
                path.display(),
                duration_ms,
                start_ms
            );
        }
        PlayerEvent::Error(e) => eprintln!("[GUI] Event Error {}", e),
        _ => {}
    }

    match event {
        PlayerEvent::Started {
            path,
            duration_ms,
            start_ms,
        } => {
            // "Started" is the engine telling us it successfully began playback.
            // We don't infer identity from path here yet.
            state.is_playing = true;
            state.duration_ms = duration_ms;
            state.position_ms = start_ms;
            state.seek_preview_ratio = None;
            state.status = format!("Now playing: {}", path.display());
        }
        PlayerEvent::Paused => state.is_playing = false,
        PlayerEvent::Resumed => state.is_playing = true,
        PlayerEvent::Stopped => {
            state.is_playing = false;
            state.position_ms = 0;
            state.duration_ms = None;
            state.seek_preview_ratio = None;
        }
        PlayerEvent::Position { position_ms } => {
            // If user is dragging the seek slider, don't fight them.
            if state.seek_preview_ratio.is_none() {
                state.position_ms = position_ms;
            }
        }
        PlayerEvent::TrackEnded => {
            state.is_playing = false;
            state.position_ms = 0;
            state.seek_preview_ratio = None;
        }
        PlayerEvent::Error(err) => {
            state.status = format!("Playback error: {err}");
        }
    }

    Task::none()
}
