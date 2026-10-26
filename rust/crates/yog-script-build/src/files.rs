//! A mod's files as its sources include them (`include_str!`): each path
//! beside its source, inside the folder the sources are named from.

use std::path::{Path, PathBuf};

use yog_script_analyze::macros::ModFiles;

/// Files beside their sources, the sources named from `base` (a mod's
/// root, the folder a tool runs in); none read from outside it.
pub struct FilesBeside {
    pub base: PathBuf,
}

impl ModFiles for FilesBeside {
    fn read(&self, source: &str, path: &str) -> Result<String, String> {
        let beside = self
            .base
            .join(source)
            .parent()
            .map_or_else(|| self.base.clone(), Path::to_path_buf)
            .join(path);
        let file = beside.canonicalize().map_err(|e| e.to_string())?;
        let base = self.base.canonicalize().map_err(|e| e.to_string())?;
        if !file.starts_with(&base) {
            return Err(format!("it's outside `{}`", base.display()));
        }
        std::fs::read_to_string(&file).map_err(|e| e.to_string())
    }
}
