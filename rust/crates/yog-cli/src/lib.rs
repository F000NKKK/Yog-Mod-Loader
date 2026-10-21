//! The `yog` CLI's internals: `yog.toml` parsing, the generated
//! `.yog-build/` workspace, `.yres` resources, `content/` blocks and items,
//! `.yui` screens and `#[yog_export]` crates.
//!
//! Not for external use — a library only so the `yog` binary, its tests and
//! its benches share the code. No API stability, not published separately.

pub mod bench;
pub(crate) mod rust_code;
pub mod book_page;
pub mod recipe_area;
pub mod cargo_toml;
pub mod content;
pub mod design;
pub mod exports;
pub mod manifest;
pub mod media;
pub mod serve;
pub mod yres;
pub mod yui;
pub mod yui_component;
pub mod yui_grid;
pub mod yui_resources;
pub mod yui_vm;

/// `path` with `/` between its parts, whatever the system's separator.
pub fn slash_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', yog_ids::sep::PATH)
}

/// `path` as a project shows it: relative to its `root`, `/`-separated.
pub(crate) fn shown_path(root: &std::path::Path, path: &std::path::Path) -> String {
    slash_path(path.strip_prefix(root).unwrap_or(path))
}

/// Every file under `dir` (in its folders too) that is `wanted`, sorted.
pub(crate) fn files_under(dir: &std::path::Path, wanted: &dyn Fn(&std::path::Path) -> bool) -> Vec<std::path::PathBuf> {
    fn walk(dir: &std::path::Path, wanted: &dyn Fn(&std::path::Path) -> bool, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
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

/// Writes generated `code` to `path` (creating its directory) unless it's
/// already there — unchanged files keep their mtime, so cargo doesn't
/// rebuild for nothing.
pub(crate) fn write_if_changed(path: &std::path::Path, code: impl AsRef<[u8]>) -> Result<(), String> {
    let code = code.as_ref();
    if std::fs::read(path).ok().as_deref() == Some(code) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, code).map_err(|e| format!("{}: {e}", path.display()))
}
