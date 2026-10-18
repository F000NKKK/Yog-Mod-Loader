//! yog-home — Yog's home on a machine (`~/.yog`, or `$YOG_HOME`).
//!
//! Three kinds of thing live there:
//! - what **Yog Installer** installed: the loader SDKs, JDKs and game
//!   instances, listed in `components.json` ([`components`]). Only the
//!   Installer writes that file.
//! - this machine's **local patch**, `local.toml` ([`local`]): written by
//!   hand, it overrides how the defaults are resolved (a loader checkout
//!   instead of the installed SDK, which JDK, the launch's environment) and
//!   adds machine-only run targets. Nothing in a mod project names a local
//!   path; it's all here.
//! - the **run targets** they give ([`targets`]): a loader on a Minecraft
//!   version, the same for every mod.

pub mod components;
pub mod local;
pub mod targets;
pub mod version;

use std::path::{Path, PathBuf};

/// The variable that moves Yog's home elsewhere.
pub const HOME_ENV: &str = "YOG_HOME";
/// Yog's home under the user's home folder.
const HOME_DIR: &str = ".yog";
/// The local patch, under the home.
const LOCAL_FILE: &str = "local.toml";
/// What Yog Installer installed, under the home.
const COMPONENTS_FILE: &str = "components.json";
/// Where Yog Installer installs, under the home: products (Yog IDLE), loader
/// SDKs, JDKs, game instances, tools (zig), and its downloads while it does.
const PRODUCTS_DIR: &str = "products";
const SDK_DIR: &str = "sdk";
const LOADER_SDK_DIR: &str = "loader";
const JDK_DIR: &str = "jdk";
const INSTANCES_DIR: &str = "instances";
const TOOLS_DIR: &str = "tools";
const CACHE_DIR: &str = "cache";
/// Where Yog Installer installs itself.
const INSTALLER_DIR: &str = "installer";

/// Yog's home folder.
#[derive(Debug, Clone)]
pub struct Home {
    root: PathBuf,
}

impl Home {
    /// `$YOG_HOME`, else `~/.yog`.
    pub fn locate() -> Result<Self, String> {
        if let Some(dir) = std::env::var_os(HOME_ENV).filter(|d| !d.is_empty()) {
            return Ok(Self::at(PathBuf::from(dir)));
        }
        dirs::home_dir()
            .map(|h| Self::at(h.join(HOME_DIR)))
            .ok_or_else(|| format!("no home folder to keep Yog's in — set {HOME_ENV}"))
    }

    pub fn at(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn local_file(&self) -> PathBuf {
        self.root.join(LOCAL_FILE)
    }

    pub fn components_file(&self) -> PathBuf {
        self.root.join(COMPONENTS_FILE)
    }

    /// A product's version: `products/<id>/<version>`.
    pub fn product_dir(&self, product: &str, version: &str) -> PathBuf {
        self.root.join(PRODUCTS_DIR).join(product).join(version)
    }

    /// A loader SDK's version: `sdk/loader/<version>`.
    pub fn loader_sdk_dir(&self, version: &str) -> PathBuf {
        self.root.join(SDK_DIR).join(LOADER_SDK_DIR).join(version)
    }

    /// A JDK: `jdk/<major>/<version>`.
    pub fn jdk_dir(&self, major: u32, version: &str) -> PathBuf {
        self.root.join(JDK_DIR).join(major.to_string()).join(version)
    }

    pub fn instances_dir(&self) -> PathBuf {
        self.root.join(INSTANCES_DIR)
    }

    /// A tool's version: `tools/<name>/<version>`.
    pub fn tool_dir(&self, name: &str, version: &str) -> PathBuf {
        self.root.join(TOOLS_DIR).join(name).join(version)
    }

    /// Where Yog Installer keeps itself.
    pub fn installer_dir(&self) -> PathBuf {
        self.root.join(INSTALLER_DIR)
    }

    /// Where downloads wait to be installed.
    pub fn cache_dir(&self) -> PathBuf {
        self.root.join(CACHE_DIR)
    }

    /// The local patch; empty when there's none.
    pub fn local(&self) -> Result<local::Local, String> {
        local::Local::read(&self.local_file(), &self.root)
    }

    /// What Yog Installer installed; nothing when it hasn't run.
    pub fn components(&self) -> Result<components::Components, String> {
        components::Components::read(&self.components_file())
    }

    /// `path` from a file in the home: `~/` is the user's home, a relative path is the home's.
    pub fn resolve(&self, path: &str) -> PathBuf {
        resolve_in(&self.root, path)
    }
}

/// `path` against `base`: `~/` is the user's home folder.
pub(crate) fn resolve_in(base: &Path, path: &str) -> PathBuf {
    let expanded = match (path.strip_prefix("~/"), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ if path == "~" => dirs::home_dir().unwrap_or_default(),
        _ => PathBuf::from(path),
    };
    if expanded.is_absolute() { expanded } else { base.join(expanded) }
}
