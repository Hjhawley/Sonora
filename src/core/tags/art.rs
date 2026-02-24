//! core/tags/art.rs
//! Read/write embedded album art (APIC/PIC) from an MP3 using the id3 crate.

use std::path::Path;

use id3::Tag;

/// Returns (image_bytes, mime) for the first embedded picture (APIC/PIC).
pub fn read_embedded_art(path: &Path) -> Result<Option<(Vec<u8>, String)>, String> {
    let tag = match Tag::read_from_path(path) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };

    // Use the crate's official picture iterator (more robust than matching frame Content).
    // This yields `&id3::frame::Picture`.
    if let Some(p) = tag.pictures().next() {
        return Ok(Some((p.data.clone(), p.mime_type.clone())));
    }

    Ok(None)
}
