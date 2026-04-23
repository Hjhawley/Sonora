//! core/tags/art.rs
//!
//! Embedded album-art IO for MP3 files using the id3 crate.
//! - Prefer front cover art when reading
//! - Fall back to the first embedded picture
//! - Only write JPEG / PNG
//! - Artwork is stored in the file tags, not in SQLite

use std::path::Path;

use id3::frame::{Content, Picture, PictureType};
use id3::{Frame, Tag, TagLike, Version};

#[derive(Debug, Clone)]
pub struct EmbeddedArt {
    pub data: Vec<u8>,
    pub mime: String,
}

pub fn read_embedded_art(path: &Path) -> Result<Option<EmbeddedArt>, String> {
    let tag = match Tag::read_from_path(path) {
        Ok(t) => t,
        // Missing/invalid tag == no art (use placeholder)
        Err(_) => return Ok(None),
    };

    let chosen = tag
        .pictures()
        .find(|p| p.picture_type == PictureType::CoverFront)
        .or_else(|| tag.pictures().next());

    Ok(chosen.map(|p| EmbeddedArt {
        data: p.data.clone(),
        mime: p.mime_type.clone(),
    }))
}

/// Replace all existing embedded artwork with a single front-cover image.
pub fn set_embedded_art(path: &Path, data: &[u8], mime: &str) -> Result<(), String> {
    let normalized_mime = normalize_supported_mime(mime)?;

    let mut tag = match Tag::read_from_path(path) {
        Ok(t) => t,
        Err(_) => Tag::new(),
    };

    remove_all_picture_frames(&mut tag);

    let picture = Picture {
        mime_type: normalized_mime.to_string(),
        picture_type: PictureType::CoverFront,
        description: String::new(),
        data: data.to_vec(),
    };

    tag.add_frame(Frame::with_content("APIC", Content::Picture(picture)));

    tag.write_to_path(path, Version::Id3v24)
        .map_err(|e| format!("Failed to write embedded artwork: {e}"))
}

/// Remove all embedded pictures. Returns 'true' if anything was removed.
pub fn remove_embedded_art(path: &Path) -> Result<bool, String> {
    let mut tag = match Tag::read_from_path(path) {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };

    let removed_any = remove_all_picture_frames(&mut tag);
    if !removed_any {
        return Ok(false);
    }

    tag.write_to_path(path, Version::Id3v24)
        .map_err(|e| format!("Failed to remove embedded artwork: {e}"))?;

    Ok(true)
}

fn normalize_supported_mime(mime: &str) -> Result<&'static str, String> {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => Ok("image/jpeg"),
        "image/png" => Ok("image/png"),
        other => Err(format!(
            "Unsupported artwork type '{other}'. Only JPEG and PNG are supported right now."
        )),
    }
}

fn remove_all_picture_frames(tag: &mut Tag) -> bool {
    let removed_apic = !tag.remove("APIC").is_empty();
    let removed_pic = !tag.remove("PIC").is_empty();
    removed_apic || removed_pic
}
