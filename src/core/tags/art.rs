//! core/tags/art.rs
//! Read embedded album art (APIC/PIC) from an MP3 using the id3 crate.

use std::path::Path;

use id3::Tag;

/// Returns '(image_bytes, mime)' for the first embedded picture (APIC/PIC).
pub fn read_embedded_art(path: &Path) -> Result<Option<(Vec<u8>, String)>, String> {
    let tag = match Tag::read_from_path(path) {
        Ok(t) => t,
        // For now, treat any tag-read failure as "no embedded art".
        // If we later need better diagnostics, distinguish missing-tag from
        // true IO / parse failures here.
        Err(_) => return Ok(None),
    };

    // Use the crate's official picture iterator rather than matching raw frame content.
    if let Some(p) = tag.pictures().next() {
        return Ok(Some((p.data.clone(), p.mime_type.clone())));
    }

    Ok(None)
}
