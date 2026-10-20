//! The `yog` CLI's internals: `yog.toml` parsing, the generated
//! `.yog-build/` workspace, `.yres` resources, `content/` blocks and items,
//! `.yui` screens and `#[yog_export]` crates.
//!
//! Not for external use — a library only so the `yog` binary, its tests and
//! its benches share the code. No API stability, not published separately.

pub mod bench;
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
pub mod yui_vm;

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
