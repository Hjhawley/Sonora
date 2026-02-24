//! core/tags/art.rs
//! Read/write embedded album art (APIC/PIC) from an MP3 using the id3 crate.
use std::path::Path;

use id3::Tag;
use id3::frame::Content;

/// Returns (image_bytes, mime) for the first embedded picture (APIC/PIC).
pub fn read_embedded_art(path: &Path) -> Result<Option<(Vec<u8>, String)>, String> {
    let tag = match Tag::read_from_path(path) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };

    for f in tag.frames() {
        if f.id() != "APIC" && f.id() != "PIC" {
            continue;
        }
        if let Content::Picture(p) = f.content() {
            // id3 crate stores mime like "image/jpeg" etc
            return Ok(Some((p.data.clone(), p.mime_type.clone())));
        }
    }

    Ok(None)
}
