//! The `yog` CLI's internals: `yog.toml` parsing, the generated
//! `.yog-build/` workspace, `.yres` resources, `content/` blocks and items,
//! `.yui` screens and `#[yog_export]` crates.
//!
//! Not for external use — a library only so the `yog` binary, its tests and
//! its benches share the code. No API stability, not published separately.

// What the binary, tests and benches reach is documented (the rest is `pub(crate)`).
#![warn(missing_docs)]

mod consts;
pub mod content;
pub mod design;
pub mod diagnostics;
pub mod files;
pub mod media;
pub mod project;
pub mod script;
pub mod serve;
pub mod ui;
pub mod yres;

/// `path` with `/` between its parts, whatever the system's separator.
pub fn slash_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', yog_ids::sep::PATH)
}

/// `path` as a project shows it: relative to its `root`, `/`-separated.
pub(crate) fn shown_path(root: &std::path::Path, path: &std::path::Path) -> String {
    slash_path(path.strip_prefix(root).unwrap_or(path))
}

/// Writes generated `code` to `path` (creating its directory) unless it's
/// already there — unchanged files keep their mtime, so cargo doesn't
/// rebuild for nothing.
pub(crate) fn write_if_changed(
    path: &std::path::Path,
    code: impl AsRef<[u8]>,
) -> Result<(), String> {
    let code = code.as_ref();
    if std::fs::read(path).ok().as_deref() == Some(code) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, code).map_err(|e| format!("{}: {e}", path.display()))
}
