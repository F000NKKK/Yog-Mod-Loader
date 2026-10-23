//! A project's files: those of a Yog format under a folder, sources read
//! and written as text, and files removed.

use std::path::{Path, PathBuf};

use yog_ids::Format;

/// Every file of `format` under `dir`, sorted.
pub(crate) fn of(dir: &Path, format: Format) -> Vec<PathBuf> {
    yog_analysis::files::with_ext(dir, &[format.ext()])
}

/// A source file of a Yog format, as text: kept binary on disk (what the
/// editor saves), read as the text the editor shows (`yog_data::source`).
pub fn read_source(path: impl AsRef<Path>) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    yog_data::source::text(&bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Writes `text` as the source file at `path`: binary, its comments kept as notes.
pub fn write_source(path: &Path, text: &str) -> Result<(), String> {
    let bytes = yog_data::source::binary(text).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// Removes `path` (a file, or a folder and all in it) that's no longer
/// wanted: already gone is fine; anything else is said, as the files stay.
pub fn remove(path: &Path) {
    let removed = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    if let Err(e) = removed
        && e.kind() != std::io::ErrorKind::NotFound
    {
        yog_logging::warn!("couldn't remove {}: {e}", path.display());
    }
}
