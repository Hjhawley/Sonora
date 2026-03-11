//! gui/update/playback.rs
//! GUI-playback engine bridge
//!
//! - `now_playing` and selection are `TrackId`, not Vec indices.
//! - `PlayTrack` accepts a `TrackId` and looks up the current row by id.
//!
//! Design goals:
//! - GUI never touches rodio/symphonia directly.
//! - All IO / timing is driven by the engine + TickPlayback polling.
//! - Engine owns continuous playback; GUI provides queue policy.

use iced::Task;
use rand::seq::SliceRandom;
use std::collections::BTreeSet;

use super::super::state::{AlbumKey, Message, PlayOrder, PlaybackContext, RepeatMode, Sonora};
use crate::core::playback::{PlayerCommand, PlayerEvent, start_playback};
use crate::core::types::TrackId;

const PREV_RESTART_THRESHOLD_MS: u64 = 3_000;

fn ensure_engine(state: &mut Sonora) {
    if state.playback.is_some() && state.playback_events.is_some() {
        return;
    }

    let (controller, events) = start_playback();
    controller.send(PlayerCommand::SetVolume(state.volume));

    state.playback = Some(controller);
    state.playback_events = Some(std::cell::RefCell::new(events));
}

fn clear_playback_ui(state: &mut Sonora) {
    state.active_playback_id = None;
    state.awaiting_started = false;
    state.now_playing = None;
    state.is_playing = false;
    state.position_ms = 0;
    state.duration_ms = None;
    state.seek_preview_ratio = None;
}

fn persist_volume(volume: f32) {
    if let Ok(db_path) = crate::core::db::default_db_path() {
        if let Ok(db) = crate::core::db::Db::open(&db_path) {
            let _ = db.save_volume(volume);
        }
    }
}

fn visible_track_ids(state: &Sonora) -> Vec<TrackId> {
    state.tracks.iter().filter_map(|t| t.id).collect()
}

fn ordered_album_track_ids(state: &Sonora, key: &AlbumKey) -> Vec<TrackId> {
    let Some(ids) = state.album_groups.get(key) else {
        return Vec::new();
    };

    let mut ids = ids.clone();

    ids.sort_by(|a, b| {
        let ta = state.track_by_id(*a);
        let tb = state.track_by_id(*b);

        match (ta, tb) {
            (Some(ta), Some(tb)) => (
                ta.disc_no.unwrap_or(0),
                ta.track_no.unwrap_or(0),
                ta.title.clone().unwrap_or_default(),
                *a,
            )
                .cmp(&(
                    tb.disc_no.unwrap_or(0),
                    tb.track_no.unwrap_or(0),
                    tb.title.clone().unwrap_or_default(),
                    *b,
                )),
            _ => a.cmp(b),
        }
    });

    ids
}

/// Returns the current ordered playback context before shuffle is applied.
///
/// This is intentionally driven by explicit playback state, not view state.
fn context_track_ids(state: &Sonora) -> Vec<TrackId> {
    match &state.playback_context {
        PlaybackContext::Library => visible_track_ids(state),
        PlaybackContext::Album(key) => {
            let ids = ordered_album_track_ids(state, key);
            if ids.is_empty() {
                visible_track_ids(state)
            } else {
                ids
            }
        }
    }
}

/// Keep only ids that are still valid for the current playback context,
/// preserve relative order, and append any newly-added ids at the end.
fn sync_shuffled_ids(state: &mut Sonora) {
    let context_ids = context_track_ids(state);
    if context_ids.is_empty() {
        state.shuffled_ids.clear();
        return;
    }

    let valid_set: BTreeSet<TrackId> = context_ids.iter().copied().collect();

    let mut seen: BTreeSet<TrackId> = BTreeSet::new();
    state
        .shuffled_ids
        .retain(|id| valid_set.contains(id) && seen.insert(*id));

    let mut queued: BTreeSet<TrackId> = state.shuffled_ids.iter().copied().collect();
    for id in context_ids {
        if queued.insert(id) {
            state.shuffled_ids.push(id);
        }
    }
}

/// Build a fresh shuffled order for the current playback context.
///
/// Behavior:
/// - shuffle only within the active playback context
/// - if there is an anchor (now playing or selected), keep it at the same index
///   it occupied in the unshuffled context when possible
fn rebuild_shuffle_order(state: &mut Sonora) {
    let mut ids = context_track_ids(state);
    if ids.is_empty() {
        state.shuffled_ids.clear();
        return;
    }

    let anchor = state.now_playing.or(state.selected_track);

    if let Some(anchor_id) = anchor {
        if let Some(anchor_idx) = ids.iter().position(|&id| id == anchor_id) {
            ids.remove(anchor_idx);
            ids.shuffle(&mut rand::thread_rng());

            let insert_at = anchor_idx.min(ids.len());
            ids.insert(insert_at, anchor_id);

            state.shuffled_ids = ids;
            return;
        }
    }

    ids.shuffle(&mut rand::thread_rng());
    state.shuffled_ids = ids;
}

fn playback_ids(state: &mut Sonora) -> Vec<TrackId> {
    match state.play_order {
        PlayOrder::Normal => context_track_ids(state),
        PlayOrder::Shuffle => {
            sync_shuffled_ids(state);
            if state.shuffled_ids.is_empty() {
                context_track_ids(state)
            } else {
                state.shuffled_ids.clone()
            }
        }
    }
}

fn next_track_id(state: &mut Sonora) -> Option<TrackId> {
    let ids = playback_ids(state);
    if ids.is_empty() {
        return None;
    }

    if state.repeat_mode == RepeatMode::One {
        return state
            .now_playing
            .or(state.selected_track)
            .filter(|id| ids.contains(id))
            .or_else(|| ids.first().copied());
    }

    let anchor_id = state.now_playing.or(state.selected_track);
    let Some(anchor_id) = anchor_id else {
        return ids.first().copied();
    };

    let Some(cur_idx) = ids.iter().position(|&id| id == anchor_id) else {
        return ids.first().copied();
    };

    if cur_idx + 1 < ids.len() {
        Some(ids[cur_idx + 1])
    } else if state.repeat_mode == RepeatMode::All {
        ids.first().copied()
    } else {
        None
    }
}

fn prev_track_id(state: &mut Sonora) -> Option<TrackId> {
    let ids = playback_ids(state);
    if ids.is_empty() {
        return None;
    }

    let anchor_id = state
        .now_playing
        .or(state.selected_track)
        .or_else(|| ids.first().copied())?;

    if state.now_playing == Some(anchor_id) && state.position_ms > PREV_RESTART_THRESHOLD_MS {
        return Some(anchor_id);
    }

    if state.repeat_mode == RepeatMode::One {
        return Some(anchor_id);
    }

    let Some(cur_idx) = ids.iter().position(|&id| id == anchor_id) else {
        return ids.first().copied();
    };

    if cur_idx > 0 {
        Some(ids[cur_idx - 1])
    } else if state.repeat_mode == RepeatMode::All {
        ids.last().copied()
    } else {
        Some(anchor_id)
    }
}

fn event_matches_active(state: &Sonora, playback_id: u64) -> bool {
    state.active_playback_id == Some(playback_id)
}

fn set_context_library(state: &mut Sonora) {
    if state.playback_context != PlaybackContext::Library {
        state.playback_context = PlaybackContext::Library;
        if state.play_order == PlayOrder::Shuffle {
            rebuild_shuffle_order(state);
        } else {
            sync_shuffled_ids(state);
        }
    }
}

fn set_context_album(state: &mut Sonora, key: AlbumKey) {
    if state.playback_context != PlaybackContext::Album(key.clone()) {
        state.playback_context = PlaybackContext::Album(key);
        if state.play_order == PlayOrder::Shuffle {
            rebuild_shuffle_order(state);
        } else {
            sync_shuffled_ids(state);
        }
    }
}

/// Tell the engine what the immediate next track should be.
/// This keeps queue policy in the GUI while continuous playback lives in the engine.
fn refresh_next_queue_hint(state: &mut Sonora) {
    let next_path = next_track_id(state)
        .and_then(|next_id| state.track_by_id(next_id))
        .map(|row| row.path.clone());

    if let Some(controller) = &state.playback {
        controller.send(PlayerCommand::ClearQueue);

        if let Some(path) = next_path {
            controller.send(PlayerCommand::QueueFile(path));
        }
    }
}

fn play_track_internal(state: &mut Sonora, id: TrackId) -> Task<Message> {
    ensure_engine(state);
    sync_shuffled_ids(state);

    let Some(row) = state.track_by_id(id) else {
        state.status = "Play failed: selected track not found (rescan?).".into();
        return Task::none();
    };

    let path = row.path.clone();

    #[cfg(debug_assertions)]
    eprintln!("[GUI] PlayTrack id={} path={}", id, path.display());

    let Some(controller) = &state.playback else {
        state.status = "Playback engine failed to initialize.".into();
        return Task::none();
    };

    controller.send(PlayerCommand::PlayFile(path.clone()));

    // Playback should not hijack selection.
    // We do not trust any older non-Started transport events after this.
    state.awaiting_started = true;
    state.now_playing = Some(id);
    state.is_playing = true;
    state.position_ms = 0;
    state.duration_ms = None;
    state.seek_preview_ratio = None;
    state.status = format!("Playing: {}", path.display());

    // Queue exactly one next track hint after current track.
    refresh_next_queue_hint(state);

    Task::none()
}

pub(crate) fn drain_events(state: &mut Sonora) -> Task<Message> {
    let Some(rx_cell) = state.playback_events.as_ref() else {
        return Task::none();
    };

    let mut drained: Vec<PlayerEvent> = Vec::new();
    {
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

    if let Some(key) = state.selected_album.clone() {
        let ids = ordered_album_track_ids(state, &key);
        if ids.contains(&id) {
            return play_album_from_track(state, key, id);
        }
    }

    play_track(state, id)
}

pub(crate) fn play_track(state: &mut Sonora, id: TrackId) -> Task<Message> {
    set_context_library(state);
    play_track_internal(state, id)
}

pub(crate) fn play_album(state: &mut Sonora, key: AlbumKey) -> Task<Message> {
    let ids = ordered_album_track_ids(state, &key);
    let Some(first_id) = ids.first().copied() else {
        state.status = "Album has no playable tracks.".to_string();
        return Task::none();
    };

    set_context_album(state, key);
    play_track_internal(state, first_id)
}

pub(crate) fn play_album_from_track(
    state: &mut Sonora,
    key: AlbumKey,
    id: TrackId,
) -> Task<Message> {
    let ids = ordered_album_track_ids(state, &key);
    if !ids.contains(&id) {
        state.status = "Track is not part of that album.".to_string();
        return Task::none();
    }

    set_context_album(state, key);
    play_track_internal(state, id)
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

pub(crate) fn toggle_shuffle(state: &mut Sonora) -> Task<Message> {
    state.play_order = match state.play_order {
        PlayOrder::Normal => {
            rebuild_shuffle_order(state);
            state.status = "Shuffle enabled.".to_string();
            PlayOrder::Shuffle
        }
        PlayOrder::Shuffle => {
            state.status = "Shuffle disabled.".to_string();
            PlayOrder::Normal
        }
    };

    refresh_next_queue_hint(state);
    Task::none()
}

pub(crate) fn cycle_repeat_mode(state: &mut Sonora) -> Task<Message> {
    state.repeat_mode = match state.repeat_mode {
        RepeatMode::Off => RepeatMode::All,
        RepeatMode::All => RepeatMode::One,
        RepeatMode::One => RepeatMode::Off,
    };

    state.status = match state.repeat_mode {
        RepeatMode::Off => "Repeat off.".to_string(),
        RepeatMode::All => "Repeat all.".to_string(),
        RepeatMode::One => "Repeat one.".to_string(),
    };

    refresh_next_queue_hint(state);
    Task::none()
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

    if let Some(controller) = &state.playback {
        controller.send(PlayerCommand::ClearQueue);
        controller.send(PlayerCommand::Stop);
    }

    clear_playback_ui(state);
    Task::none()
}

pub(crate) fn next(state: &mut Sonora) -> Task<Message> {
    if state.tracks.is_empty() {
        return Task::none();
    }

    let Some(next_id) = next_track_id(state) else {
        state.status = "End of queue.".to_string();
        return stop(state);
    };

    play_track_internal(state, next_id)
}

pub(crate) fn prev(state: &mut Sonora) -> Task<Message> {
    if state.tracks.is_empty() {
        return Task::none();
    }

    let Some(prev_id) = prev_track_id(state) else {
        state.status = "No playable track found.".to_string();
        return Task::none();
    };

    play_track_internal(state, prev_id)
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

    // Seeking to the end tends to produce EOF weirdness, so clamp slightly.
    if target_ms >= dur_ms {
        target_ms = dur_ms.saturating_sub(1);
    }

    #[cfg(debug_assertions)]
    eprintln!(
        "[GUI] SeekCommit ratio={} dur_ms={} => target_ms={}",
        ratio, dur_ms, target_ms
    );

    state.awaiting_started = true;
    controller.send(PlayerCommand::ClearQueue);
    controller.send(PlayerCommand::Seek(target_ms));

    state.position_ms = target_ms;

    // Refresh the one-track lookahead after seek.
    refresh_next_queue_hint(state);

    Task::none()
}

pub(crate) fn set_volume(state: &mut Sonora, volume: f32) -> Task<Message> {
    let volume = volume.clamp(0.0, 1.0);
    state.volume = volume;

    if let Some(controller) = &state.playback {
        controller.send(PlayerCommand::SetVolume(volume));
    }

    persist_volume(volume);

    Task::none()
}

pub(crate) fn handle_event(state: &mut Sonora, event: PlayerEvent) -> Task<Message> {
    #[cfg(debug_assertions)]
    match &event {
        PlayerEvent::Started {
            playback_id,
            path,
            duration_ms,
            start_ms,
        } => {
            eprintln!(
                "[GUI] Event Started playback_id={} path={} duration_ms={:?} start_ms={}",
                playback_id,
                path.display(),
                duration_ms,
                start_ms
            );
        }
        PlayerEvent::Paused { playback_id } => {
            eprintln!("[GUI] Event Paused playback_id={}", playback_id);
        }
        PlayerEvent::Resumed { playback_id } => {
            eprintln!("[GUI] Event Resumed playback_id={}", playback_id);
        }
        PlayerEvent::Stopped { playback_id } => {
            eprintln!("[GUI] Event Stopped playback_id={}", playback_id);
        }
        PlayerEvent::Position {
            playback_id,
            position_ms,
        } => {
            eprintln!(
                "[GUI] Event Position playback_id={} position_ms={}",
                playback_id, position_ms
            );
        }
        PlayerEvent::TrackEnded { playback_id } => {
            eprintln!("[GUI] Event TrackEnded playback_id={}", playback_id);
        }
        PlayerEvent::Error(e) => eprintln!("[GUI] Event Error {}", e),
    }

    match event {
        PlayerEvent::Started {
            playback_id,
            path,
            duration_ms,
            start_ms,
        } => {
            state.active_playback_id = Some(playback_id);
            state.awaiting_started = false;
            state.is_playing = true;
            state.duration_ms = duration_ms;
            state.position_ms = start_ms;
            state.seek_preview_ratio = None;
            state.status = format!("Now playing: {}", path.display());

            // Whenever engine advances into a new queued track, compute and queue
            // the next logical track so continuous playback can keep going.
            refresh_next_queue_hint(state);
        }

        PlayerEvent::Paused { playback_id } => {
            if state.awaiting_started || !event_matches_active(state, playback_id) {
                return Task::none();
            }
            state.is_playing = false;
        }

        PlayerEvent::Resumed { playback_id } => {
            if state.awaiting_started || !event_matches_active(state, playback_id) {
                return Task::none();
            }
            state.is_playing = true;
        }

        PlayerEvent::Stopped { playback_id } => {
            if state.awaiting_started || !event_matches_active(state, playback_id) {
                return Task::none();
            }

            clear_playback_ui(state);
        }

        PlayerEvent::Position {
            playback_id,
            position_ms,
        } => {
            if state.awaiting_started || !event_matches_active(state, playback_id) {
                return Task::none();
            }

            if state.seek_preview_ratio.is_none() {
                state.position_ms = position_ms;
            }
        }

        PlayerEvent::TrackEnded { playback_id } => {
            if state.awaiting_started || !event_matches_active(state, playback_id) {
                return Task::none();
            }

            clear_playback_ui(state);
            state.status = "Reached end of queue.".to_string();
        }

        PlayerEvent::Error(err) => {
            state.awaiting_started = false;
            state.status = format!("Playback error: {err}");
        }
    }

    Task::none()
}
