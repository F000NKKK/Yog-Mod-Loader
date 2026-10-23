//! A mod project as the designers see it: the manifest at its root and the
//! mod a file is in.

use std::path::{Path, PathBuf};

pub(crate) use crate::media::assets::mod_id;

/// `<root>/yog.toml`.
pub(crate) fn path(root: &Path) -> PathBuf {
    root.join(yog_ids::file::MANIFEST)
}

/// The mod `file` belongs to: the nearest folder (itself included) with a `yog.toml`.
pub(crate) fn mod_root(file: &Path) -> Option<PathBuf> {
    file.ancestors()
        .find(|d| path(d).is_file())
        .map(Path::to_path_buf)
}
