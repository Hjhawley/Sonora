use std::path::{Path, PathBuf};

pub fn scan_mp3s(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    walk_dir(root, &mut out)?;
    Ok(out)
}

fn walk_dir(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{dir:?}: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            walk_dir(&path, out)?;
        } else if is_mp3(&path) {
            out.push(path);
        }
    }

    Ok(())
}

fn is_mp3(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("mp3"))
        .unwrap_or(false)
}
