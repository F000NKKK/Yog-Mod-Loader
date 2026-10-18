//! A project's files, by format.

use std::path::{Path, PathBuf};

use yog_ids::Format;

/// Every file under `dir` (all the way down) whose extension is `ext`, sorted.
pub fn with_ext(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == ext) {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Every file of `format` under `dir`, sorted.
pub fn of(dir: &Path, format: Format) -> Vec<PathBuf> {
    with_ext(dir, format.ext())
}
