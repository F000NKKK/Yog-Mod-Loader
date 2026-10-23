//! Yog's file formats and where files live.

use crate::{kind, ns, res, sep, yog};

macro_rules! formats {
    ($( $(#[$doc:meta])* $variant:ident = $ext:expr, $noun:expr; )*) => {
        /// Yog's file formats: each one's extension and what a file of it is
        /// called in messages. What each looks like is `yog_formats::shape`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Format { $( $(#[$doc])* $variant ),* }

        impl Format {
            /// Every format.
            pub const ALL: &'static [Format] = &[$(Format::$variant),*];

            /// Its extension, without the dot: `yblock`.
            #[must_use]
            pub const fn ext(self) -> &'static str {
                match self { $(Format::$variant => $ext),* }
            }

            /// What one file of it is: `block`, `animation`.
            #[must_use]
            pub const fn noun(self) -> &'static str {
                match self { $(Format::$variant => $noun),* }
            }
        }
    };
}

formats! {
    Block = "yblock", kind::BLOCK;
    Item = "yitem", kind::ITEM;
    Entity = "yentity", kind::ENTITY;
    Recipe = "yrecipe", kind::RECIPE;
    Book = "ybook", kind::BOOK;
    /// A definition of some kind (`kind: "ladder"`).
    Def = "ydef", "definition";
    /// Named resources: strings, colours, sounds, numbers.
    Res = "yres", "resources";
    /// A screen or a component.
    Ui = "yui", "screen";
    Model = "ymodel", "model";
    Anim = "yanim", "animation";
    Font = "yfont", "font";
    /// A texture: its picture as given and how it animates.
    Texture = "ytexture", kind::TEXTURE;
    /// A sound event: the sounds it plays, and how.
    Sound = "ysound", kind::SOUND;
    /// A particle: its pictures, how it moves and how it's drawn.
    Particle = "yparticle", kind::PARTICLE;
    /// A video: what it shows (a source or a montage), its size and frame rate.
    Video = "yvideo", kind::VIDEO;
    /// A material: the textures a surface is drawn with, and how.
    Material = "ymaterial", kind::MATERIAL;
    /// Compiled Yog Data.
    Data = "ydata", "compiled data";
    /// A built mod.
    Mod = "yog", "mod";
    /// Yog Script: a mod's code.
    Script = "ys", "script";
    /// Yog Nanite Script: Yog Script that patches others' classes.
    Nanites = "yns", "nanite script";
}

impl Format {
    /// The format with extension `ext` (with or without the dot).
    #[must_use]
    pub fn of(ext: &str) -> Option<Format> {
        let ext = ext.trim_start_matches('.');
        Format::ALL.iter().copied().find(|f| f.ext() == ext)
    }

    /// The format of the file at `path`, by its extension.
    #[must_use]
    pub fn of_path(path: &std::path::Path) -> Option<Format> {
        Format::of(path.extension()?.to_str()?)
    }

    /// Whether `path` is a file of this format.
    #[must_use]
    pub fn is(self, path: &std::path::Path) -> bool {
        Format::of_path(path) == Some(self)
    }

    /// Whether its files are Yog Data — kept binary on disk, shown as text
    /// (every format but `.yui` markup and built `.yog` mods).
    #[must_use]
    pub const fn is_data(self) -> bool {
        !matches!(
            self,
            Format::Ui | Format::Mod | Format::Script | Format::Nanites
        )
    }

    /// `name` with this format's extension: `stone` → `stone.yblock`.
    #[must_use]
    pub fn file(self, name: &str) -> String {
        format!("{name}.{}", self.ext())
    }
}

/// Texture `path`'s file under its namespace: `block/stone` → `textures/block/stone.png`.
#[must_use]
pub fn texture_file(path: &str) -> String {
    format!("{}{path}{}", res::TEXTURES, res::PNG)
}

/// The texture a file under its namespace is: `textures/block/stone.png` →
/// `block/stone`; none for another file.
#[must_use]
pub fn texture_of_file(file: &str) -> Option<&str> {
    file.strip_prefix(res::TEXTURES)?.strip_suffix(res::PNG)
}

/// Model `path`'s file: `item/apple` → `models/item/apple.json`.
#[must_use]
pub fn model_file(path: &str) -> String {
    format!("{}{path}{}", res::MODELS, res::JSON)
}

/// Item `path`'s definition (1.21.4+): `apple` → `items/apple.json`.
#[must_use]
pub fn item_definition_file(path: &str) -> String {
    format!("{}{path}{}", res::ITEM_DEFINITIONS, res::JSON)
}

/// Block `path`'s states' file: `stone` → `blockstates/stone.json`.
#[must_use]
pub fn blockstate_file(path: &str) -> String {
    format!("{}{path}{}", res::BLOCKSTATES, res::JSON)
}

/// The registries of entities and biomes, which aren't named as their kind.
const ENTITY_REGISTRY: &str = crate::renamed_tag_dir::ENTITY_TYPE;
const BIOME_REGISTRY: &str = crate::concatcp!(crate::pack::WORLDGEN, sep::PATH, kind::BIOME);

/// Asset `rel` of namespace `ns`'s, as the pack holds it: `assets/<ns>/<rel>`.
#[must_use]
pub fn asset_file(ns: &str, rel: &str) -> String {
    format!("{}{}{ns}{}{rel}", crate::pack::ASSETS, sep::PATH, sep::PATH)
}

/// Crate `name` as Rust code names it: `yog-api` → `yog_api`.
#[must_use]
pub fn rust_name(name: &str) -> String {
    name.replace(sep::HYPHEN, sep::WORD)
}

/// The game's registry of content `kind` — what its tags' and data's
/// folders are named: `entity` → `entity_type`, `biome` → `worldgen/biome`.
#[must_use]
pub fn registry_of(kind: &str) -> &str {
    match kind {
        kind::ENTITY => ENTITY_REGISTRY,
        kind::BIOME => BIOME_REGISTRY,
        other => other,
    }
}

/// Whether folder `dir` (a data or tag folder) was plural before 1.21.
fn renamed_in_1_21(dir: &str) -> bool {
    crate::renamed_data_dir::ALL_VALUES.contains(&dir)
        || crate::renamed_tag_dir::ALL_VALUES.contains(&dir)
}

/// The folders of content `kind`'s data or tags, the current one first, then
/// 1.20's plural one when it was named differently: `item` → `item`, `items`.
#[must_use]
pub fn registry_dirs(kind: &str) -> Vec<String> {
    let dir = registry_of(kind);
    let mut out = vec![dir.to_string()];
    if renamed_in_1_21(dir) {
        out.push(format!("{dir}{}", crate::data_layout::PLURAL));
    }
    out
}

/// Language `locale`'s file under a namespace: `lang/en_us.json`.
#[must_use]
pub fn lang_file(locale: &str) -> String {
    format!("{}{locale}{}", res::LANG, res::JSON)
}

/// What the game's translation keys of content `kind` in namespace `ns` start with: `item.minecraft.`.
#[must_use]
pub fn lang_prefix(kind: &str, ns: &str) -> String {
    format!("{kind}.{ns}.")
}

/// The game's translation key of content `kind` `id`: `item.minecraft.iron_ingot`, a path's `/` as `.`.
#[must_use]
pub fn lang_key(kind: &str, id: &str) -> String {
    let (ns, path) = split(id);
    format!("{}{}", lang_prefix(kind, ns), path.replace(sep::PATH, "."))
}

/// `ns:path`.
pub fn join(ns: &str, path: &str) -> String {
    format!("{ns}{}{path}", sep::NAMESPACE)
}

/// A namespace as the game takes it: `[a-z0-9_.-]+`.
pub fn valid_namespace(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'.' | b'-')
        })
}

/// A resource path as the game takes it: `[a-z0-9_./-]+`, not starting or
/// ending with a `/`.
pub fn valid_path(s: &str) -> bool {
    let sep = sep::PATH.as_bytes()[0];
    !s.is_empty()
        && !s.starts_with(sep::PATH)
        && !s.ends_with(sep::PATH)
        && s.bytes().all(|b| {
            b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || matches!(b, b'_' | b'.' | b'-')
                || b == sep
        })
}

/// `ns:path` (or `path`, the game's) as the game takes it.
pub fn valid_id(s: &str) -> bool {
    match s.split_once(sep::NAMESPACE) {
        Some((ns, path)) => valid_namespace(ns) && valid_path(path),
        None => valid_path(s),
    }
}

/// A name of Yog's in snake case: `[a-z0-9_]+` (a state's property, a kind of recipe).
pub fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == sep::WORD.as_bytes()[0])
}

/// `id`'s namespace and path; a bare path is the game's.
pub fn split(id: &str) -> (&str, &str) {
    id.split_once(sep::NAMESPACE).unwrap_or((ns::MINECRAFT, id))
}

/// The path of the game's `id` (`minecraft:stone` → `stone`); none for
/// another namespace's, or a bare path.
pub fn game_path(id: &str) -> Option<&str> {
    id.split_once(sep::NAMESPACE)
        .and_then(|(n, path)| (n == ns::MINECRAFT).then_some(path))
}

/// `id` with a namespace: the game's if it has none.
pub fn qualify(id: &str) -> String {
    qualify_in(ns::MINECRAFT, id)
}

/// `id` with a namespace: `ns` if it has none (what a mod writes bare is its own).
pub fn qualify_in(ns: &str, id: &str) -> String {
    if id.contains(sep::NAMESPACE) {
        id.to_string()
    } else {
        join(ns, id)
    }
}

/// The game font `font` stands for when it's Yog's alias of one
/// (`yog:font/uniform` → `minecraft:uniform`).
#[must_use]
pub fn game_font_of(font: &str) -> Option<String> {
    font.strip_prefix(yog::FONT_ALIAS)
        .filter(|name| !name.is_empty())
        .map(|name| join(ns::MINECRAFT, name))
}

/// Yog's alias of the game's font `name`: `yog:font/<name>`.
#[must_use]
pub fn font_alias(name: &str) -> String {
    format!("{}{name}", yog::FONT_ALIAS)
}
