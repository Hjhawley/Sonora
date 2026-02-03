use std::path::PathBuf;

use id3::{Tag, TagLike};

use super::types::TrackRow;

pub fn read_track_row(path: PathBuf) -> (TrackRow, bool) {
    match Tag::read_from_path(&path) {
        Ok(tag) => (
            TrackRow {
                path,
                title: tag.title().map(str::to_owned),
                artist: tag.artist().map(str::to_owned),
                album: tag.album().map(str::to_owned),
                track_no: tag.track(),
                year: tag.year(),
            },
            false,
        ),
        Err(_) => (
            TrackRow {
                path,
                title: None,
                artist: None,
                album: None,
                track_no: None,
                year: None,
            },
            true,
        ),
    }
}
