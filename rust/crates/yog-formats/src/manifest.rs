//! `yog.toml` — a mod project's manifest (TOML, read into Yog Data).

use yog_data::{Ordered, Schema, Text, Value};

/// `[mod]`.
#[derive(Schema)]
pub struct ModSection {
    /// The mod's id — its namespace (`mymod`).
    pub id: String,
    /// Shown name: text or a `.yres` text (`@Strings.mod_name`).
    pub name: Option<Text>,
    pub version: Option<String>,
    pub description: Option<Text>,
    /// An image source or a `.yres` image (`@Icons.logo`).
    pub icon: Option<yog_data::Image>,
    #[data(default)]
    pub authors: Vec<String>,
    pub license: Option<String>,
    /// License of the generated exports crate (default MIT OR Apache-2.0).
    pub exports_license: Option<String>,
    /// The Minecraft version whose vanilla content the mod names (its
    /// datapack: `1.21.1`) — the newest Yog supports by default. On other
    /// versions renamed content is mapped (`short_grass` is `grass` on 1.20.1).
    pub datapack: Option<String>,
    /// Rust edition (default 2024).
    pub edition: Option<String>,
}

/// `[dev]`.
#[derive(Schema)]
pub struct DevSection {
    /// A local yog-api checkout to build against.
    pub yog_api_path: Option<String>,
}

/// `[run."<name>"]`: a dev instance `yog run <name>` builds into and starts.
#[derive(Schema)]
pub struct RunSection {
    pub name: Option<String>,
    pub export_dir: Option<String>,
    pub command: Option<String>,
    #[data(default)]
    pub args: Vec<String>,
    pub cwd: Option<String>,
    /// `KEY=VALUE` pairs.
    #[data(default)]
    pub env: Vec<String>,
}

/// `[external.<namespace>]`: the other mods' content this mod names.
#[derive(Schema)]
pub struct ExternalSection {
    #[data(default)]
    pub items: Vec<String>,
    #[data(default)]
    pub blocks: Vec<String>,
    #[data(default)]
    pub entities: Vec<String>,
    #[data(default)]
    pub recipes: Vec<String>,
}

/// A whole `yog.toml`.
#[derive(Schema)]
pub struct ManifestFile {
    #[data(rename = "mod")]
    pub module: ModSection,
    pub dev: Option<DevSection>,
    /// Crates as Cargo writes them; `exports` is the other Yog mods'.
    #[data(default)]
    pub dependencies: Ordered<Value>,
    #[data(rename = "dev-dependencies", default)]
    pub dev_dependencies: Ordered<Value>,
    #[data(default)]
    pub run: Ordered<RunSection>,
    #[data(default)]
    pub external: Ordered<ExternalSection>,
}
