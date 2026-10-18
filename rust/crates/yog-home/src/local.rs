//! `local.toml` — this machine's patch over Yog's defaults, written by hand:
//!
//! ```toml
//! [sdk]
//! loader = "~/Projects/yog/Yog-Mod-Loader"   # a checkout instead of the installed SDK
//!
//! [java]                                      # a JDK per Java version
//! 21 = "/usr/lib/jvm/java-21-openjdk-amd64"
//!
//! [launch]                                    # every launch's environment
//! env = ["__NV_PRIME_RENDER_OFFLOAD=1", "__GLX_VENDOR_LIBRARY_NAME=nvidia"]
//!
//! [run."prism"]                               # a target only this machine has
//! name       = "Prism: The Suspect's Journey"
//! export_dir = "~/.local/share/PrismLauncher/instances/x/minecraft/yog-mods"
//! command    = "prismlauncher"
//! args       = ["--launch", "x"]
//! ```
//!
//! Paths are the user's home's with `~/`, else the Yog home's.

use crate::resolve_in;
use std::path::{Path, PathBuf};
use yog_data::{Ordered, Schema};

/// `[sdk]`: which source tree each SDK is.
#[derive(Schema)]
pub struct SdkSection {
    /// A Yog Mod Loader checkout.
    pub loader: Option<String>,
}

/// `[launch]`: what every launch gets.
#[derive(Schema)]
pub struct LaunchSection {
    /// `KEY=VALUE` pairs.
    #[data(default)]
    pub env: Vec<String>,
}

/// `[run."<id>"]`: a target only this machine has — a game instance of its own.
#[derive(Schema, Clone, Debug)]
pub struct RunSection {
    pub name: Option<String>,
    /// Where the built `.yog` goes (an instance's `yog-mods`).
    pub export_dir: Option<String>,
    /// What starts the game, after the export (none: export only).
    pub command: Option<String>,
    #[data(default)]
    pub args: Vec<String>,
    pub cwd: Option<String>,
    /// `KEY=VALUE` pairs.
    #[data(default)]
    pub env: Vec<String>,
}

/// A whole `local.toml`.
#[derive(Schema)]
pub struct LocalFile {
    pub sdk: Option<SdkSection>,
    /// Java's major version → that JDK's folder.
    #[data(default)]
    pub java: Ordered<String>,
    pub launch: Option<LaunchSection>,
    #[data(default)]
    pub run: Ordered<RunSection>,
}

/// The local patch, its paths resolved.
#[derive(Default, Clone)]
pub struct Local {
    pub loader_sdk: Option<PathBuf>,
    /// Java's major version → its JDK.
    pub java: Vec<(u32, PathBuf)>,
    pub env: Vec<(String, String)>,
    pub runs: Vec<(String, RunSection)>,
    /// Where relative paths in it are from.
    pub base: PathBuf,
}

impl Local {
    /// `path` (relative paths from `base`); empty when there's none.
    pub fn read(path: &Path, base: &Path) -> Result<Self, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self { base: base.to_path_buf(), ..Self::default() }),
            Err(e) => return Err(format!("reading {}: {e}", path.display())),
        };
        let at = |message: String| format!("{}: {message}", path.display());
        let file = yog_data::schema::from_toml::<LocalFile>(&text).map_err(|e| at(e.to_string()))?.value;
        let mut java = Vec::new();
        for (major, dir) in file.java.0 {
            let major = major.parse().map_err(|_| at(format!("[java]: `{major}` isn't a Java version (17, 21, …)")))?;
            java.push((major, resolve_in(base, &dir)));
        }
        Ok(Self {
            loader_sdk: file.sdk.and_then(|s| s.loader).map(|p| resolve_in(base, &p)),
            java,
            env: file.launch.map(|l| pairs(&l.env)).unwrap_or_default(),
            runs: file.run.0,
            base: base.to_path_buf(),
        })
    }

    /// The JDK it names for Java `major`.
    pub fn jdk(&self, major: u32) -> Option<&Path> {
        self.java.iter().find(|(m, _)| *m == major).map(|(_, p)| p.as_path())
    }
}

/// `KEY=VALUE` strings as pairs (one without `=` is skipped).
pub fn pairs(env: &[String]) -> Vec<(String, String)> {
    env.iter().filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect()
}
