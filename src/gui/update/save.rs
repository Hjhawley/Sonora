use iced::Task;

use super::super::state::{Message, Sonora};
use super::super::util::{parse_optional_i32, parse_optional_u32};
use super::helpers::spawn_blocking;
use super::inspector::load_inspector_from_track;

pub(crate) fn save_inspector_to_file(state: &mut Sonora) -> Task<Message> {
    if state.scanning || state.saving {
        return Task::none();
    }

    if !state.inspector_dirty {
        state.status = "No changes to save.".to_string();
        return Task::none();
    }

    let Some(i) = state.selected_track else {
        state.status = "Select a track first.".to_string();
        return Task::none();
    };
    if i >= state.tracks.len() {
        state.status = "Invalid selection (rescan?).".to_string();
        return Task::none();
    }

    let row_to_write = match build_row_from_inspector(state, i) {
        Ok(r) => r,
        Err(e) => {
            state.status = e;
            return Task::none();
        }
    };

    state.saving = true;
    state.status = "Writing tags to file...".to_string();
    let write_extended = state.show_extended;

    Task::perform(
        spawn_blocking(move || {
            crate::core::tags::write_track_row(&row_to_write, write_extended).and_then(|_| {
                let (r, failed) = crate::core::tags::read_track_row(row_to_write.path.clone());
                if failed {
                    Err("Wrote tags, but failed to re-read them".to_string())
                } else {
                    Ok(r)
                }
            })
        }),
        move |res| Message::SaveFinished(i, res),
    )
}

pub(crate) fn save_finished(
    state: &mut Sonora,
    i: usize,
    result: Result<crate::core::types::TrackRow, String>,
) -> Task<Message> {
    state.saving = false;

    match result {
        Ok(new_row) => {
            if i < state.tracks.len() {
                state.tracks[i] = new_row;
                load_inspector_from_track(state);
            }
            state.inspector_dirty = false;
            state.status = "Tags written to file.".to_string();
        }
        Err(e) => {
            state.status = format!("Save failed: {e}");
        }
    }

    Task::none()
}

pub(crate) fn revert_inspector(state: &mut Sonora) -> Task<Message> {
    load_inspector_from_track(state);
    Task::none()
}

fn build_row_from_inspector(
    state: &Sonora,
    i: usize,
) -> Result<crate::core::types::TrackRow, String> {
    let mut out = state
        .tracks
        .get(i)
        .cloned()
        .ok_or_else(|| "Invalid selection (rescan?).".to_string())?;

    // Parse numeric fields (collect invalid labels; we don't auto-correct user input).
    let mut errs: Vec<&'static str> = Vec::new();

    let track_no = parse_optional_u32(&state.inspector.track_no)
        .inspect_err(|_| errs.push("Track #"))
        .ok()
        .flatten();

    let track_total = parse_optional_u32(&state.inspector.track_total)
        .inspect_err(|_| errs.push("Track total"))
        .ok()
        .flatten();

    let disc_no = parse_optional_u32(&state.inspector.disc_no)
        .inspect_err(|_| errs.push("Disc #"))
        .ok()
        .flatten();

    let disc_total = parse_optional_u32(&state.inspector.disc_total)
        .inspect_err(|_| errs.push("Disc total"))
        .ok()
        .flatten();

    let year = parse_optional_i32(&state.inspector.year)
        .inspect_err(|_| errs.push("Year"))
        .ok()
        .flatten();

    // BPM is extended now, so only validate/overwrite when extended is visible.
    let bpm = if state.show_extended {
        parse_optional_u32(&state.inspector.bpm)
            .inspect_err(|_| errs.push("BPM"))
            .ok()
            .flatten()
    } else {
        out.bpm
    };

    if !errs.is_empty() {
        return Err(format!("Not saved: invalid {}", errs.join(", ")));
    }

    // -------------------------
    // Standard (always applied)
    // -------------------------
    out.title = clean_opt(&state.inspector.title);
    out.artist = clean_opt(&state.inspector.artist);
    out.album = clean_opt(&state.inspector.album);
    out.album_artist = clean_opt(&state.inspector.album_artist);
    out.composer = clean_opt(&state.inspector.composer);

    out.track_no = track_no;
    out.track_total = track_total;
    out.disc_no = disc_no;
    out.disc_total = disc_total;

    out.year = year;
    out.genre = clean_opt(&state.inspector.genre);

    out.grouping = clean_opt(&state.inspector.grouping);
    out.comment = clean_opt(&state.inspector.comment);
    out.lyrics = clean_opt(&state.inspector.lyrics);
    out.lyricist = clean_opt(&state.inspector.lyricist);

    // -------------------------
    // Extended (only if visible)
    // -------------------------
    if state.show_extended {
        out.date = clean_opt(&state.inspector.date);

        out.conductor = clean_opt(&state.inspector.conductor);
        out.remixer = clean_opt(&state.inspector.remixer);
        out.publisher = clean_opt(&state.inspector.publisher);
        out.subtitle = clean_opt(&state.inspector.subtitle);

        out.bpm = bpm;
        out.key = clean_opt(&state.inspector.key);
        out.mood = clean_opt(&state.inspector.mood);
        out.language = clean_opt(&state.inspector.language);
        out.isrc = clean_opt(&state.inspector.isrc);
        out.encoder_settings = clean_opt(&state.inspector.encoder_settings);
        out.encoded_by = clean_opt(&state.inspector.encoded_by);
        out.copyright = clean_opt(&state.inspector.copyright);
    }

    Ok(out)
}

/// Trim user input; return `None` if the result is empty.
fn clean_opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}
