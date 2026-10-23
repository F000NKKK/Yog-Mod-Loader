//! Writing generated files.

use std::io;
use std::path::Path;

/// Writes `code` to `path` (creating its directory) unless it's already
/// there: unchanged files keep their mtime, so builds don't redo work.
/// Whether it wrote.
pub fn write_if_changed(path: &Path, code: impl AsRef<[u8]>) -> io::Result<bool> {
    let code = code.as_ref();
    if std::fs::read(path).ok().as_deref() == Some(code) {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, code)?;
    Ok(true)
}
