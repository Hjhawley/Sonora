//! core/library.rs
//!
//! Read-only filesystem discovery.
//!
//! This module:
//! - walks configured directory trees
//! - identifies supported audio files
//! - collects lightweight filesystem facts
//!
//! Discovery must either complete successfully or return an error. Permission
//! failures are not silently skipped because reconciliation assumes that a
//! successful discovery result represents the complete scanned filesystem
//! state.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Lightweight filesystem facts collected during discovery.
///
/// 'mtime_unix' stores nanoseconds since the Unix epoch. The field name is
/// retained for compatibility with the existing reconciliation boundary.
#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub mtime_unix: Option<i64>,
    pub size: Option<u64>,
}

/// Recursively scan a directory tree for supported audio files.
///
/// Behavior:
/// - the root must exist and resolve to a directory
/// - permission and traversal failures abort the scan
/// - entries that disappear during traversal are skipped
/// - symlinked directories are not traversed
/// - symlinked files are included when they resolve to supported files
/// - path identity is lexical rather than canonical
/// - output is sorted by full path
///
/// Because identity is lexical, a file reached through both its real path and
/// a symlink may appear as two distinct library entries.
pub fn scan_audio_files(root: &Path) -> Result<Vec<DiscoveredFile>, String> {
    let root_metadata = std::fs::metadata(root).map_err(|e| format!("{}: {e}", root.display()))?;

    if !root_metadata.is_dir() {
        return Err(format!("Not a directory: {}", root.display()));
    }

    let mut discovered = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(directory) = stack.pop() {
        // A directory failure makes the discovery result incomplete, so do
        // not continue into reconciliation with a partial result.
        let entries =
            std::fs::read_dir(&directory).map_err(|e| format!("{}: {e}", directory.display()))?;

        for entry_result in entries {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(error) if is_disappeared_entry(&error) => continue,
                Err(error) => {
                    return Err(format!("{}: {error}", directory.display()));
                }
            };

            let path = entry.path();

            // 'file_type' does not follow symlinks.
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) if is_disappeared_entry(&error) => continue,
                Err(error) => {
                    return Err(format!("{}: {error}", path.display()));
                }
            };

            if file_type.is_dir() {
                stack.push(path);
                continue;
            }

            if file_type.is_symlink() {
                let metadata = match std::fs::metadata(&path) {
                    Ok(metadata) => metadata,
                    Err(error) if is_disappeared_entry(&error) => continue,
                    Err(error) => {
                        return Err(format!("{}: {error}", path.display()));
                    }
                };

                if metadata.is_file() && is_supported_audio_file(&path) {
                    discovered.push(discovered_from_metadata(path, &metadata));
                }

                // Symlinked directories are intentionally not traversed.
                continue;
            }

            if file_type.is_file() && is_supported_audio_file(&path) {
                let metadata = match entry.metadata() {
                    Ok(metadata) => metadata,
                    Err(error) if is_disappeared_entry(&error) => continue,
                    Err(error) => {
                        return Err(format!("{}: {error}", path.display()));
                    }
                };

                discovered.push(discovered_from_metadata(path, &metadata));
            }
        }
    }

    discovered.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(discovered)
}

fn discovered_from_metadata(path: PathBuf, metadata: &std::fs::Metadata) -> DiscoveredFile {
    let mtime_unix = metadata.modified().ok().and_then(system_time_to_unix_nanos);

    DiscoveredFile {
        path,
        mtime_unix,
        size: Some(metadata.len()),
    }
}

fn system_time_to_unix_nanos(time: SystemTime) -> Option<i64> {
    let elapsed = time.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(elapsed.as_nanos()).ok()
}

/// An entry can legitimately disappear between 'read_dir' and inspection.
/// Skipping that entry still represents its final observed state accurately.
fn is_disappeared_entry(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::NotFound
}

/// Return whether a path has a currently supported audio extension.
///
/// Sonora currently supports MP3. FLAC and M4A support can be added here when
/// their complete read/write behavior is implemented elsewhere.
fn is_supported_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mp3"))
}
