use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TrackRow {
    pub path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_no: Option<u32>,
}
