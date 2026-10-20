//! Yog's file formats and where files live.

use crate::*;

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
    /// A material: the textures a surface is drawn with, and how.
    Material = "ymaterial", kind::MATERIAL;
    /// Compiled Yog Data.
    Data = "ydata", "compiled data";
    /// A built mod.
    Mod = "yog", "mod";
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

/// Model `path`'s file: `item/apple` → `models/item/apple.json`.
#[must_use]
pub fn model_file(path: &str) -> String {
    format!("{}{path}{}", res::MODELS, res::JSON)
}

/// Block `path`'s states' file: `stone` → `blockstates/stone.json`.
#[must_use]
pub fn blockstate_file(path: &str) -> String {
    format!("{}{path}{}", res::BLOCKSTATES, res::JSON)
}

/// The game's registry of content `kind` — what its tags' and data's
/// folders are named: `entity` → `entity_type`, `biome` → `worldgen/biome`.
#[must_use]
pub fn registry_of(kind: &str) -> &str {
    match kind {
        kind::ENTITY => "entity_type",
        kind::BIOME => "worldgen/biome",
        other => other,
    }
}

/// The kinds whose data and tag folders were plural before 1.21 (`items`, `loot_tables`).
const PLURAL_BEFORE_1_21: &[&str] = &[kind::ITEM, kind::BLOCK, kind::ENTITY, kind::FLUID, kind::LOOT_TABLE, kind::ADVANCEMENT, kind::RECIPE];

/// The folders of content `kind`'s data or tags, the current one first, then
/// 1.20's plural one when it was named differently: `item` → `item`, `items`.
#[must_use]
pub fn registry_dirs(kind: &str) -> Vec<String> {
    let dir = registry_of(kind);
    let mut out = vec![dir.to_string()];
    if PLURAL_BEFORE_1_21.contains(&kind) {
        out.push(format!("{dir}s"));
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

/// `id`'s namespace and path; a bare path is the game's.
pub fn split(id: &str) -> (&str, &str) {
    id.split_once(sep::NAMESPACE).unwrap_or((ns::MINECRAFT, id))
}

/// `id` with a namespace: the game's if it has none.
pub fn qualify(id: &str) -> String {
    qualify_in(ns::MINECRAFT, id)
}

/// `id` with a namespace: `ns` if it has none (what a mod writes bare is its own).
pub fn qualify_in(ns: &str, id: &str) -> String {
    if id.contains(sep::NAMESPACE) { id.to_string() } else { join(ns, id) }
}

/// The game font `font` stands for when it's Yog's alias of one
/// (`yog:font/uniform` → `minecraft:uniform`).
#[must_use]
pub fn game_font_of(font: &str) -> Option<String> {
    font.strip_prefix(yog::FONT_ALIAS).filter(|name| !name.is_empty()).map(|name| join(ns::MINECRAFT, name))
}

/// Yog's alias of the game's font `name`: `yog:font/<name>`.
#[must_use]
pub fn font_alias(name: &str) -> String {
    format!("{}{name}", yog::FONT_ALIAS)
}
