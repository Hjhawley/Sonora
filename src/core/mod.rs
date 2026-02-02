pub mod library;
pub mod tags;
pub mod types;

use std::collections::HashSet;
use std::path::PathBuf;

use types::TrackRow;

pub fn scan_and_read_roots(roots: Vec<PathBuf>) -> Result<(Vec<TrackRow>, usize), String> {
    let mut rows = Vec::new();
    let mut tag_failures = 0usize;
    let mut seen = HashSet::<PathBuf>::new();

    for root in roots {
        let paths = library::scan_mp3s(&root)?;

        for path in paths {
            if !seen.insert(path.clone()) {
                continue; // avoid duplicates if roots overlap
            }

            let (row, failed) = tags::read_track_row(path);
            if failed {
                tag_failures += 1;
            }
            rows.push(row);
        }
    }

    Ok((rows, tag_failures))
}
