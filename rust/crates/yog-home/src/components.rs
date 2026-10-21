//! `components.json` — what Yog Installer installed, and where. Only the
//! Installer writes it; everything else reads it.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The ids of the components the rest of Yog looks for.
pub mod id {
    /// Yog Installer itself — it records where it is, for the others to start it.
    pub const INSTALLER: &str = "yog.installer";
    /// A Yog Mod Loader source tree: its loaders and versions, built and run with its `build.sh`.
    pub const LOADER_SDK: &str = "yog.sdk.loader";
    /// zig, the linker `yog build` cross-links every platform with (a folder holding `zig`).
    pub const ZIG: &str = "zig";
    /// The prefix of a JDK's id: `yog.jdk.21`.
    pub const JDK_PREFIX: &str = "yog.jdk.";

    /// The JDK of Java `major`.
    pub fn jdk(major: u32) -> String {
        format!("{JDK_PREFIX}{major}")
    }
}

/// One installed component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installed {
    pub id: String,
    pub version: String,
    /// Where it is (absolute).
    pub path: PathBuf,
}

/// Everything installed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Components {
    #[serde(default)]
    pub installed: Vec<Installed>,
}

impl Components {
    /// `path`'s components; none when the file isn't there.
    pub fn read(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("reading {}: {e}", path.display())),
        }
    }

    /// Writes them to `path` (the Installer's part).
    pub fn write(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("writing {}: {e}", path.display()))
    }

    /// The newest installed `id` (versions compared part by part).
    pub fn newest(&self, id: &str) -> Option<&Installed> {
        self.installed
            .iter()
            .filter(|c| c.id == id)
            .max_by(|a, b| crate::version::compare(&a.version, &b.version))
    }

    /// Installs `c`, replacing the same id and version.
    pub fn put(&mut self, c: Installed) {
        self.installed
            .retain(|i| !(i.id == c.id && i.version == c.version));
        self.installed.push(c);
    }

    /// Forgets `id` at `version`.
    pub fn remove(&mut self, id: &str, version: &str) {
        self.installed
            .retain(|i| !(i.id == id && i.version == version));
    }
}
