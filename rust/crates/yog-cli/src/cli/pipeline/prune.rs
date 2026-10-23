//! Only the current build kept in cargo's target folder: after a build,
//! every unit a previous one left (an older dependency, other features, an
//! older toolchain) is removed, so `obj/target` doesn't grow build by build.
//!
//! Cargo names a unit's files and folders `<name>-<hash>`; a build's
//! messages name every file it used, fresh or rebuilt. Whatever carries a
//! hash none of them has is stale.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The folders of a profile whose entries are units, by hash.
const UNIT_DIRS: [&str; 4] = ["deps", "build", ".fingerprint", "incremental"];
/// How long cargo's unit hashes are, in hex digits.
const HASH_LEN: usize = 16;
/// Cargo's messages naming the files a unit made, and the folder a build
/// script wrote.
const ARTIFACT: &str = "compiler-artifact";
const BUILD_SCRIPT: &str = "build-script-executed";
const REASON: &str = "reason";

/// What a build used: its units' hashes, and the profile folders they're in.
#[derive(Default)]
pub(super) struct Used {
    hashes: HashSet<String>,
    profiles: HashSet<PathBuf>,
}

impl Used {
    /// Takes in what one of cargo's JSON messages names.
    pub(super) fn note(&mut self, v: &Value) {
        let paths: Vec<&str> = match v.get(REASON).and_then(Value::as_str) {
            Some(ARTIFACT) => v
                .get("filenames")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .chain(v.get("executable").and_then(Value::as_str))
                .collect(),
            Some(BUILD_SCRIPT) => v
                .get("out_dir")
                .and_then(Value::as_str)
                .into_iter()
                .collect(),
            _ => return,
        };
        for path in paths {
            self.path(Path::new(path));
        }
    }

    /// A used file: the hash of the unit folder or file it's in, and its profile.
    fn path(&mut self, path: &Path) {
        let parts: Vec<_> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect();
        for (i, part) in parts.iter().enumerate() {
            if UNIT_DIRS.contains(&part.as_ref()) {
                self.profiles.insert(path.components().take(i).collect());
                if let Some(h) = parts.get(i + 1).and_then(|n| hash_of(n)) {
                    self.hashes.insert(h.to_string());
                }
            }
        }
    }

    /// Removes, in each profile used, every unit with a hash the build didn't use.
    pub(super) fn prune(&self) {
        if self.hashes.is_empty() {
            return;
        }
        for profile in &self.profiles {
            for dir in UNIT_DIRS {
                let Ok(entries) = std::fs::read_dir(profile.join(dir)) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let stale =
                        hash_of(&name.to_string_lossy()).is_some_and(|h| !self.hashes.contains(h));
                    if stale {
                        yog_cli::files::remove(&entry.path());
                    }
                }
            }
        }
    }
}

/// The hash in a unit's file or folder name: `libfoo-0123456789abcdef.rlib`,
/// `foo-0123456789abcdef`.
fn hash_of(name: &str) -> Option<&str> {
    let stem = name.split('.').next()?;
    let (_, hash) = stem.rsplit_once('-')?;
    (hash.len() == HASH_LEN && hash.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hash)
}

#[cfg(test)]
mod tests;
