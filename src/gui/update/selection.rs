//! gui/update/selection.rs
//!
//! Selection interactions + album interactions + cover loading.
//! - All selection is keyed by 'TrackId' (stable), not 'Vec' indices.
//! - Album View:
//!   - grid when 'selected_album == None'
//!   - album detail screen when 'selected_album == Some(...)'

use iced::Task;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::super::state::{AlbumKey, AlbumPressTarget, Message, Sonora, ViewMode};
use super::inspector::{clear_inspector, load_inspector_from_selection};
use super::playback;
use super::util::spawn_blocking;
use crate::core;
use crate::core::types::TrackId;

const DOUBLE_CLICK_WINDOW_MS: u64 = 400;

pub(crate) fn select_album(state: &mut Sonora, key: AlbumKey) -> Task<Message> {
    if state.view_mode != ViewMode::Albums {
        state.view_mode = ViewMode::Albums;
    }

    state.selected_album = Some(key.clone());
    state.selected_tracks.clear();

    let ordered_ids = ordered_album_track_ids(state, &key);

    if ordered_ids.is_empty() {
        state.selected_track = None;
        state.selection_anchor = None;
        state.last_clicked_track = None;
        state.inspector_open = false;
        clear_inspector(state);
        return Task::none();
    }

    for &id in &ordered_ids {
        state.selected_tracks.insert(id);
    }

    let primary_id = ordered_ids[0];
    state.selected_track = Some(primary_id);
    state.selection_anchor = Some(primary_id);
    state.last_clicked_track = Some(primary_id);

    load_inspector_from_selection(state);

    let mut preload_tasks: Vec<Task<Message>> = Vec::new();
    for &id in &ordered_ids {
        preload_tasks.push(maybe_load_cover_for_track(state, id));
    }

    Task::batch(preload_tasks)
}

pub(crate) fn track_pressed(state: &mut Sonora, id: TrackId) -> Task<Message> {
    let shift = state.modifiers.shift();
    let ctrl = state.modifiers.control();

    if shift {
        return select_range_to_track(state, id);
    }

    if ctrl {
        return toggle_track_selection(state, id);
    }

    if state.selected_track == Some(id) && state.selected_tracks.len() <= 1 {
        return playback::play_track(state, id);
    }

    select_single_track(state, id)
}

pub(crate) fn album_tile_pressed(state: &mut Sonora, key: AlbumKey) -> Task<Message> {
    let is_double = register_album_press(state, AlbumPressTarget::Tile(key.clone()));

    let select_task = select_album(state, key.clone());

    if is_double {
        let play_task = playback::play_album(state, key);
        Task::batch(vec![select_task, play_task])
    } else {
        select_task
    }
}

pub(crate) fn album_header_pressed(state: &mut Sonora, key: AlbumKey) -> Task<Message> {
    let is_double = register_album_press(state, AlbumPressTarget::Header(key.clone()));

    let select_task = select_album(state, key.clone());

    if is_double {
        let play_task = playback::play_album(state, key);
        Task::batch(vec![select_task, play_task])
    } else {
        select_task
    }
}

pub(crate) fn album_track_pressed(state: &mut Sonora, key: AlbumKey, id: TrackId) -> Task<Message> {
    let is_double = register_album_press(state, AlbumPressTarget::Track(key.clone(), id));

    let select_task = track_pressed(state, id);

    if is_double {
        let play_task = playback::play_album_from_track(state, key, id);
        Task::batch(vec![select_task, play_task])
    } else {
        select_task
    }
}

pub(crate) fn select_adjacent_track(
    state: &mut Sonora,
    delta: isize,
    extend: bool,
) -> Task<Message> {
    let ids = ordered_selectable_track_ids(state);
    if ids.is_empty() {
        return Task::none();
    }

    let current = state
        .selected_track
        .or_else(|| state.selected_tracks.iter().next().copied());

    let current_index = current
        .and_then(|id| ids.iter().position(|&x| x == id))
        .unwrap_or(if delta >= 0 {
            0
        } else {
            ids.len().saturating_sub(1)
        });

    let next_index = if delta < 0 {
        current_index.saturating_sub(delta.unsigned_abs())
    } else {
        (current_index + delta as usize).min(ids.len().saturating_sub(1))
    };

    let next_id = ids[next_index];

    if extend {
        select_range_to_track(state, next_id)
    } else {
        select_single_track(state, next_id)
    }
}

pub(crate) fn select_all_in_context(state: &mut Sonora) -> Task<Message> {
    let ids = ordered_selectable_track_ids(state);

    if ids.is_empty() {
        return Task::none();
    }

    state.selected_tracks.clear();
    for &id in &ids {
        state.selected_tracks.insert(id);
    }

    state.selected_track = ids.first().copied();
    state.selection_anchor = state.selected_track;
    state.last_clicked_track = ids.last().copied();

    load_inspector_from_selection(state);

    if let Some(primary_id) = state.selected_track {
        maybe_load_cover_for_track(state, primary_id)
    } else {
        Task::none()
    }
}

pub(crate) fn clear_selection(state: &mut Sonora) -> Task<Message> {
    clear_selection_and_inspector(state);
    Task::none()
}

pub(crate) fn cover_loaded(
    state: &mut Sonora,
    id: TrackId,
    handle: Option<iced::widget::image::Handle>,
) -> Task<Message> {
    if let Some(h) = handle {
        state.cover_cache.insert(id, h);
    } else {
        state.cover_cache.remove(&id);
    }
    Task::none()
}

/// Preload representative cover art for album grid tiles / album view transitions.
pub(crate) fn preload_album_covers(state: &mut Sonora) -> Task<Message> {
    let rep_ids: Vec<TrackId> = state
        .album_groups
        .keys()
        .filter_map(|key| state.representative_cover_track_id(key))
        .collect();

    let mut tasks: Vec<Task<Message>> = Vec::new();
    for id in rep_ids {
        tasks.push(maybe_load_cover_for_track(state, id));
    }

    Task::batch(tasks)
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

fn ordered_selectable_track_ids(state: &Sonora) -> Vec<TrackId> {
    match state.view_mode {
        ViewMode::Tracks => state.track_view_ids.clone(),
        ViewMode::Albums => {
            if let Some(key) = &state.selected_album {
                ordered_album_track_ids(state, key)
            } else {
                Vec::new()
            }
        }
    }
}

fn select_single_track(state: &mut Sonora, id: TrackId) -> Task<Message> {
    let Some(idx) = state.index_of_id(id) else {
        return Task::none();
    };

    if state.view_mode == ViewMode::Albums {
        let clicked_key = album_key_for_index(state, idx);

        let keep_album_open = state.selected_album.as_ref().is_some_and(|k| {
            k.album_artist == clicked_key.album_artist && k.album == clicked_key.album
        });

        if !keep_album_open {
            state.selected_album = None;
        }
    } else {
        state.selected_album = None;
    }

    state.selected_tracks.clear();
    state.selected_tracks.insert(id);
    state.selected_track = Some(id);
    state.selection_anchor = Some(id);
    state.last_clicked_track = Some(id);

    load_inspector_from_selection(state);

    maybe_load_cover_for_track(state, id)
}

fn toggle_track_selection(state: &mut Sonora, id: TrackId) -> Task<Message> {
    let Some(idx) = state.index_of_id(id) else {
        return Task::none();
    };

    if state.view_mode == ViewMode::Albums {
        let clicked_key = album_key_for_index(state, idx);

        let keep_album_open = state.selected_album.as_ref().is_some_and(|k| {
            k.album_artist == clicked_key.album_artist && k.album == clicked_key.album
        });

        if !keep_album_open {
            state.selected_album = None;
        }
    } else {
        state.selected_album = None;
    }

    if state.selected_tracks.contains(&id) {
        state.selected_tracks.remove(&id);
        if state.selected_track == Some(id) {
            state.selected_track = state.selected_tracks.iter().next_back().copied();
        }
    } else {
        state.selected_tracks.insert(id);
        state.selected_track = Some(id);
        state.selection_anchor = Some(id);
    }

    state.last_clicked_track = Some(id);

    if state.has_selection() {
        load_inspector_from_selection(state);
    } else {
        state.inspector_open = false;
        clear_inspector(state);
    }

    maybe_load_cover_for_track(state, id)
}

fn select_range_to_track(state: &mut Sonora, id: TrackId) -> Task<Message> {
    let ids = ordered_selectable_track_ids(state);
    if ids.is_empty() {
        return select_single_track(state, id);
    }

    let anchor = state
        .selection_anchor
        .or(state.selected_track)
        .unwrap_or(id);

    let Some(anchor_idx) = ids.iter().position(|&x| x == anchor) else {
        return select_single_track(state, id);
    };
    let Some(target_idx) = ids.iter().position(|&x| x == id) else {
        return select_single_track(state, id);
    };

    let start = anchor_idx.min(target_idx);
    let end = anchor_idx.max(target_idx);

    state.selected_tracks.clear();
    for &track_id in &ids[start..=end] {
        state.selected_tracks.insert(track_id);
    }

    state.selected_track = Some(id);
    state.last_clicked_track = Some(id);

    load_inspector_from_selection(state);
    maybe_load_cover_for_track(state, id)
}

fn register_album_press(state: &mut Sonora, target: AlbumPressTarget) -> bool {
    let now = Instant::now();
    let window = Duration::from_millis(DOUBLE_CLICK_WINDOW_MS);

    let is_double = state
        .last_album_press
        .as_ref()
        .is_some_and(|(prev, at)| *prev == target && now.duration_since(*at) <= window);

    state.last_album_press = Some((target, now));
    is_double
}

fn album_key_for_index(state: &Sonora, idx: usize) -> AlbumKey {
    let t = &state.tracks[idx];

    let album_artist = t
        .album_artist
        .clone()
        .or_else(|| t.artist.clone())
        .unwrap_or_else(|| "Unknown Artist".to_string());

    let album = t
        .album
        .clone()
        .unwrap_or_else(|| "Unknown Album".to_string());

    AlbumKey {
        album_artist,
        album,
    }
}

fn maybe_load_cover_for_track(state: &mut Sonora, id: TrackId) -> Task<Message> {
    if state.cover_cache.contains_key(&id) {
        return Task::none();
    }

    let Some(track) = state.track_by_id(id) else {
        return Task::none();
    };

    if track.artwork_count == 0 {
        return Task::none();
    }

    let path: PathBuf = track.path.clone();

    Task::perform(
        spawn_blocking(move || load_cover_handle_from_path(&path)),
        move |handle| Message::CoverLoaded(id, handle),
    )
}

fn load_cover_handle_from_path(path: &Path) -> Option<iced::widget::image::Handle> {
    let art = core::tags::read_embedded_art(path).ok()??;
    Some(iced::widget::image::Handle::from_bytes(art.data))
}

pub(crate) fn clear_selection_and_inspector(state: &mut Sonora) {
    state.selected_track = None;
    state.selected_tracks.clear();
    state.selection_anchor = None;
    state.last_clicked_track = None;
    state.selected_album = None;
    state.last_album_press = None;
    state.inspector_open = false;

    clear_inspector(state);
}
