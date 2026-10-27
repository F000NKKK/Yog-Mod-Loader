//! `yog.toml` — a mod project's manifest (TOML, read into Yog Data).

use yog_data::{Ordered, Schema, Text, Value};

/// `[mod]`.
#[derive(Schema)]
pub struct ModSection {
    /// The mod's id — its namespace (`mymod`).
    pub id: String,
    /// Shown name: text or a `.yres` text (`@res:Strings.mod_name`).
    pub name: Option<Text>,
    pub version: Option<String>,
    pub description: Option<Text>,
    /// An image source or a `.yres` image (`@res:Icons.logo`).
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
    /// The language the tools default to for the mod's code (a new
    /// `.yui`'s code-behind, handlers designers write, `yog new`'s
    /// sources): Yog Script unless it says Rust. Both always build.
    #[data(default)]
    pub language: Language,
}

/// A mod's main language: a preference of the tools, never a limit.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    /// Yog Script (`.ys`, `.yns`).
    #[default]
    Ys,
    /// Rust (`src/lib.rs`).
    Rust,
}

/// `[dev]`.
#[derive(Schema)]
pub struct DevSection {
    /// A local yog-api checkout to build against.
    pub yog_api_path: Option<String>,
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

/// `[network]`: where the mod sends — said, so code that sends without
/// saying so doesn't build, and a server knows what its clients need.
#[derive(Schema, Default)]
pub struct NetworkSection {
    /// It sends to the server (client → server): it then needs Yog on the
    /// server too, so a client with it can't join one without.
    #[data(default)]
    pub to_server: bool,
}

/// A whole `yog.toml`.
#[derive(Schema)]
pub struct ManifestFile {
    #[data(rename = "mod")]
    pub module: ModSection,
    pub dev: Option<DevSection>,
    pub network: Option<NetworkSection>,
    /// Crates as Cargo writes them; `exports` is the other Yog mods'.
    #[data(default)]
    pub dependencies: Ordered<Value>,
    #[data(rename = "dev-dependencies", default)]
    pub dev_dependencies: Ordered<Value>,
    #[data(default)]
    pub external: Ordered<ExternalSection>,
}

/// What a built `.yog` says about its mod (`yog_ids::archive::MANIFEST`):
/// what the loader shows of it and orders the mods by, without the project.
#[derive(Schema, Debug, Clone, Default, PartialEq)]
pub struct Built {
    pub id: String,
    pub name: String,
    pub version: String,
    #[data(default)]
    pub description: String,
    #[data(default)]
    pub authors: Vec<String>,
    #[data(default)]
    pub license: String,
    /// The platforms it has natives for.
    #[data(default)]
    pub platforms: Vec<String>,
    /// The translation keys of `name` and `description`, when they're `.yres` texts.
    pub name_key: Option<String>,
    pub description_key: Option<String>,
    /// Its icon, an image source.
    pub icon: Option<String>,
    /// The Yog mods it needs loaded before it, by id.
    #[data(default)]
    pub dependencies: Vec<String>,
    /// It sends to the server (`[network] to_server`): it needs Yog there.
    #[data(default)]
    pub to_server: bool,
}
