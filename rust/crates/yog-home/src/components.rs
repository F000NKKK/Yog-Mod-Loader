//! `components.ydata` — what Yog Installer installed, and where (binary Yog
//! Data). Only the Installer writes it; everything else reads it.

use std::path::{Path, PathBuf};
use yog_data::Schema;

/// The ids of the components the rest of Yog looks for.
pub mod id {
    /// Yog Installer itself — it records where it is, for the others to start it.
    pub const INSTALLER: &str = "yog.installer";
    /// A Yog Mod Loader source tree: its loaders and versions, built and run with its `build.sh`.
    pub const LOADER_SDK: &str = "yog.sdk.loader";
    /// zig, the linker `yog build` cross-links every platform with (a folder holding `zig`).
    pub const ZIG: &str = "zig";
    /// The loader runtime's debug info, split from its release (`symbols/<platform>/<lib>.debug`).
    pub const RUNTIME_SYMBOLS: &str = "yog.symbols.runtime";
    /// The prefix of a JDK's id: `yog.jdk.21`.
    pub const JDK_PREFIX: &str = "yog.jdk.";

    /// The JDK of Java `major`.
    pub fn jdk(major: u32) -> String {
        format!("{JDK_PREFIX}{major}")
    }
}

/// One installed component.
#[derive(Debug, Clone, PartialEq, Eq, Schema)]
pub struct Installed {
    pub id: String,
    pub version: String,
    /// Where it is (absolute).
    pub path: PathBuf,
}

/// Everything installed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Schema)]
pub struct Components {
    #[data(default)]
    pub installed: Vec<Installed>,
}

impl Components {
    /// `path`'s components; none when the file isn't there.
    pub fn read(path: &Path) -> Result<Self, String> {
        match std::fs::read(path) {
            Ok(bytes) => {
                yog_data::read_bytes(&bytes).map_err(|e| format!("{}: {e:?}", path.display()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("reading {}: {e}", path.display())),
        }
    }

    /// Writes them to `path` (the Installer's part).
    pub fn write(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        }
        std::fs::write(path, yog_data::to_bytes(self))
            .map_err(|e| format!("writing {}: {e}", path.display()))
    }

    /// The newest installed `id` (versions compared part by part).
    pub fn newest(&self, id: &str) -> Option<&Installed> {
        self.installed
            .iter()
            .filter(|c| c.id == id)
            .max_by(|a, b| crate::version::compare(&a.version, &b.version))
    }

    /// The installed runtime symbols' debug info for library `lib` of
    /// platform `tag` (`<component>/<tag>/<lib>.debug`), if it's there.
    pub fn runtime_symbols(&self, tag: &str, lib: &str) -> Option<PathBuf> {
        let base = &self.newest(id::RUNTIME_SYMBOLS)?.path;
        let file = base.join(tag).join(format!(
            "{lib}{}{}",
            yog_ids::sep::DOT,
            yog_ids::symbols::EXT
        ));
        file.is_file().then_some(file)
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
