//! gui/update/art.rs
//! Inspector artwork actions + generic cover loading helpers.

use std::fs;
use std::path::{Path, PathBuf};

use iced::Task;
use iced::widget::image;
use rfd::FileDialog;

use super::super::state::{ArtworkEdit, Message, PickedArtwork, Sonora};
use super::util::spawn_blocking;
use crate::core::types::TrackId;

pub(crate) fn reset_inspector_artwork_state(state: &mut Sonora) {
    state.inspector_art_edit = ArtworkEdit::Unchanged;
}

pub(crate) fn load_cover_for_track(id: TrackId, path: PathBuf) -> Task<Message> {
    Task::perform(
        spawn_blocking(move || load_cover_handle(&path)),
        move |handle| Message::CoverLoaded(id, handle),
    )
}

pub(crate) fn choose_inspector_artwork(state: &mut Sonora) -> Task<Message> {
    if state.scanning || state.saving {
        return Task::none();
    }

    if !state.has_selection() {
        state.status = "Select one or more tracks first.".to_string();
        return Task::none();
    }

    Task::perform(
        spawn_blocking(pick_artwork_from_dialog),
        Message::InspectorArtworkChosen,
    )
}

pub(crate) fn inspector_artwork_chosen(
    state: &mut Sonora,
    result: Result<Option<PickedArtwork>, String>,
) -> Task<Message> {
    match result {
        Ok(Some(picked)) => {
            let preview = image::Handle::from_bytes(picked.bytes.clone());

            state.inspector_art_edit = ArtworkEdit::Replace {
                bytes: picked.bytes,
                mime: picked.mime,
                preview,
            };
            state.inspector_dirty = true;
            state.status = "Artwork queued for save.".to_string();
        }
        Ok(None) => {
            // dialog canceled
        }
        Err(e) => {
            state.status = format!("Artwork load failed: {e}");
        }
    }

    Task::none()
}

pub(crate) fn remove_inspector_artwork(state: &mut Sonora) -> Task<Message> {
    if state.scanning || state.saving {
        return Task::none();
    }

    if !state.has_selection() {
        state.status = "Select one or more tracks first.".to_string();
        return Task::none();
    }

    state.inspector_art_edit = ArtworkEdit::Remove;
    state.inspector_dirty = true;
    state.status = "Artwork removal queued for save.".to_string();

    Task::none()
}

pub(crate) fn extract_inspector_artwork(state: &mut Sonora) -> Task<Message> {
    if state.scanning || state.saving {
        return Task::none();
    }

    let Some(id) = state.selected_track else {
        state.status = "Extract artwork works on a single selected track.".to_string();
        return Task::none();
    };

    if !state.selected_tracks.is_empty() && state.selected_tracks.len() > 1 {
        state.status = "Extract artwork works on a single selected track.".to_string();
        return Task::none();
    }

    let Some(row) = state.track_by_id(id) else {
        state.status = "Selected track not found.".to_string();
        return Task::none();
    };

    let path = row.path.clone();
    let suggested_name = row
        .title
        .clone()
        .or_else(|| {
            row.path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "cover".to_string());

    let pending_replace = match &state.inspector_art_edit {
        ArtworkEdit::Replace { bytes, mime, .. } => Some(PickedArtwork {
            bytes: bytes.clone(),
            mime: mime.clone(),
        }),
        _ => None,
    };

    Task::perform(
        spawn_blocking(move || extract_artwork_via_dialog(path, suggested_name, pending_replace)),
        Message::InspectorArtworkExtracted,
    )
}

pub(crate) fn inspector_artwork_extracted(
    state: &mut Sonora,
    result: Result<Option<PathBuf>, String>,
) -> Task<Message> {
    match result {
        Ok(Some(path)) => {
            state.status = format!("Artwork extracted to {}.", path.display());
        }
        Ok(None) => {
            // dialog canceled
        }
        Err(e) => {
            state.status = format!("Artwork extract failed: {e}");
        }
    }

    Task::none()
}

fn load_cover_handle(path: &Path) -> Option<image::Handle> {
    match crate::core::tags::read_embedded_art(path) {
        Ok(Some(art)) => Some(image::Handle::from_bytes(art.data)),
        _ => None,
    }
}

fn pick_artwork_from_dialog() -> Result<Option<PickedArtwork>, String> {
    let Some(path) = FileDialog::new()
        .add_filter("Artwork", &["jpg", "jpeg", "png"])
        .pick_file()
    else {
        return Ok(None);
    };

    let bytes = fs::read(&path)
        .map_err(|e| format!("Failed to read artwork file '{}': {e}", path.display()))?;
    let mime = detect_supported_image_mime(&bytes, &path)?;

    Ok(Some(PickedArtwork { bytes, mime }))
}

fn extract_artwork_via_dialog(
    track_path: PathBuf,
    suggested_base_name: String,
    pending_replace: Option<PickedArtwork>,
) -> Result<Option<PathBuf>, String> {
    let art = if let Some(pending) = pending_replace {
        pending
    } else {
        let Some(existing) = crate::core::tags::read_embedded_art(&track_path)? else {
            return Err("No embedded artwork found.".to_string());
        };

        PickedArtwork {
            bytes: existing.data,
            mime: existing.mime,
        }
    };

    let ext = extension_for_mime(&art.mime);
    let safe_name = sanitize_filename(&suggested_base_name);
    let default_name = format!("{safe_name}.{ext}");

    let Some(out_path) = FileDialog::new().set_file_name(&default_name).save_file() else {
        return Ok(None);
    };

    fs::write(&out_path, art.bytes).map_err(|e| {
        format!(
            "Failed to write extracted artwork to '{}': {e}",
            out_path.display()
        )
    })?;

    Ok(Some(out_path))
}

fn detect_supported_image_mime(bytes: &[u8], path: &Path) -> Result<String, String> {
    if is_png(bytes) {
        return Ok("image/png".to_string());
    }

    if is_jpeg(bytes) {
        return Ok("image/jpeg".to_string());
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match ext.as_str() {
        "png" => Ok("image/png".to_string()),
        "jpg" | "jpeg" => Ok("image/jpeg".to_string()),
        _ => Err("Unsupported image type. Only JPEG and PNG are supported right now.".to_string()),
    }
}

fn extension_for_mime(mime: &str) -> &'static str {
    match mime.to_ascii_lowercase().as_str() {
        "image/png" => "png",
        _ => "jpg",
    }
}

fn is_png(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes[..8] == [137, 80, 78, 71, 13, 10, 26, 10]
}

fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF
}

fn sanitize_filename(s: &str) -> String {
    let trimmed = s.trim();
    let base = if trimmed.is_empty() { "cover" } else { trimmed };

    base.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => c,
        })
        .collect()
}
