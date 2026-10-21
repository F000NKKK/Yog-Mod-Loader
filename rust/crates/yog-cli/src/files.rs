//! A project's files: those under a folder, by what they are, and a
//! fingerprint of a set of them that changes when any of them does.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use yog_ids::Format;

/// Every file under `dir` (in its folders too) that is `wanted`, sorted.
pub(crate) fn under(dir: &Path, wanted: &dyn Fn(&Path) -> bool) -> Vec<PathBuf> {
    fn walk(dir: &Path, wanted: &dyn Fn(&Path) -> bool, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for path in entries.flatten().map(|e| e.path()) {
            if path.is_dir() {
                walk(&path, wanted, out);
            } else if wanted(&path) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, wanted, &mut out);
    out.sort();
    out
}

/// Every file under `dir` whose extension is `ext`, sorted.
pub fn with_ext(dir: &Path, ext: &str) -> Vec<PathBuf> {
    under(dir, &|p| p.extension().is_some_and(|e| e == ext))
}

/// Every file of `format` under `dir`, sorted.
pub fn of(dir: &Path, format: Format) -> Vec<PathBuf> {
    with_ext(dir, format.ext())
}

/// A hash of `files`' paths, sizes and change times: it changes when one of them does.
pub(crate) fn fingerprint(files: &[PathBuf]) -> u64 {
    let mut h = DefaultHasher::new();
    for f in files {
        f.hash(&mut h);
        if let Ok(m) = std::fs::metadata(f) {
            m.len().hash(&mut h);
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .hash(&mut h);
        }
    }
    h.finish()
}
