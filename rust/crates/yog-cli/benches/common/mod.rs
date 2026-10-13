//! Fixtures shared by the benches: copies of HexMod-Yog's and Yog-Modlist's
//! resources, content and screens under `benches/fixtures/`.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// `benches/fixtures/<name>`.
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("benches/fixtures")
        .join(name)
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// A fixture mod as a project on disk: a copy in a temp dir, with an empty
/// file for every path in its `placeholders.txt` (textures the fixtures
/// refer to, left out of the repo — only their presence is checked).
pub fn mod_root(name: &str) -> PathBuf {
    let src = fixture(name);
    let dst = std::env::temp_dir().join(format!("yog-cli-bench-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&src, &dst);
    if let Ok(list) = std::fs::read_to_string(src.join("placeholders.txt")) {
        for rel in list.lines().filter(|l| !l.is_empty()) {
            let path = dst.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"").unwrap();
        }
    }
    dst
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap().flatten() {
        let to = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), to).unwrap();
        }
    }
}
