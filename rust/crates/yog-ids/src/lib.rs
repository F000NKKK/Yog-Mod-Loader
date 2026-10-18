//! yog-ids — the names every part of Yog shares, defined once: namespaces,
//! kinds of content, typed-value words, image-source schemes, separators and
//! Yog's own ids. The lowest crate: everything above takes these from here
//! (composing its own constants with [`concatcp!`] / [`id!`]); the IDE
//! (TypeScript) and the hosts (Java) get them generated ([`export`]).

pub use const_format::concatcp;

/// `id!(ns::YOG, "state")` → `"yog:state"`, at compile time.
#[macro_export]
macro_rules! id {
    ($ns:expr, $path:expr) => {
        $crate::concatcp!($ns, $crate::sep::NAMESPACE, $path)
    };
}

macro_rules! names {
    ($( $(#[$gdoc:meta])* $group:ident { $( $(#[$doc:meta])* $name:ident = $value:expr ),* $(,)? } )*) => {
        $(
            $(#[$gdoc])*
            pub mod $group {
                #[allow(unused_imports)] // a group composes names from the ones above
                use super::*;
                $( $(#[$doc])* pub const $name: &str = $value; )*
            }
        )*
        /// Every name, by group — what [`export`] writes.
        pub const ALL: &[(&str, &[(&str, &str)])] = &[ $( (stringify!($group), &[ $( (stringify!($name), $group::$name) ),* ]) ),* ];
    };
}

names! {
    /// Namespaces.
    ns {
        /// The game's: ids without a namespace are its.
        MINECRAFT = "minecraft",
        /// Yog's own: its tags, its channels, its aliases of the game's content.
        YOG = "yog",
    }

    /// What separates the parts of names.
    sep {
        /// A namespace from its path: `minecraft:stone`.
        NAMESPACE = ":",
        /// A path's parts: `textures/block/stone`.
        PATH = "/",
        /// What starts a typed value: `@item:minecraft:stick`.
        TYPED = "@",
        /// What marks a tag where an ingredient is shown: `#c:dusts`.
        TAG_SHOWN = "#",
        /// What joins an ingredient's alternatives where it's shown: `a | b`.
        ALTERNATIVES = "|",
    }

    /// The kinds of content: what a reference names (`@item:…`), what a
    /// `.ydef` defines (`kind: "ladder"`), what a mod registers with the
    /// host (`content("potion", …)`).
    kind {
        ITEM = "item",
        BLOCK = "block",
        ENTITY = "entity",
        RECIPE = "recipe",
        ENTRY = "entry",
        CATEGORY = "category",
        BOOK = "book",
        FLUID = "fluid",
        BIOME = "biome",
        BIOME_FEATURE = "biome_feature",
        DIMENSION = "dimension",
        ATTRIBUTE = "attribute",
        EFFECT = "effect",
        POTION = "potion",
        INVENTORY = "inventory",
        STARTUP_GRANT = "startup_grant",
        ADVANCEMENT = "advancement",
        LADDER = "ladder",
        LEVEL = "level",
        SOUND_GROUP = "sound_group",
        RECIPE_KIND = "recipe_kind",
        TOOL_MATERIAL = "tool_material",
        ARMOR_MATERIAL = "armor_material",
        ORE = "ore",
        TABLE = "table",
        TAG = "tag",
        DAMAGE_TYPE = "damage_type",
        TREE = "tree",
        LOOT_MODIFIER = "loot_modifier",
        LOOT_TABLE = "loot_table",
    }

    /// What a reference to content of a kind starts with: `@item:`.
    reference {
        ITEM = concatcp!(sep::TYPED, kind::ITEM, sep::NAMESPACE),
        BLOCK = concatcp!(sep::TYPED, kind::BLOCK, sep::NAMESPACE),
        ENTITY = concatcp!(sep::TYPED, kind::ENTITY, sep::NAMESPACE),
        RECIPE = concatcp!(sep::TYPED, kind::RECIPE, sep::NAMESPACE),
        ENTRY = concatcp!(sep::TYPED, kind::ENTRY, sep::NAMESPACE),
        CATEGORY = concatcp!(sep::TYPED, kind::CATEGORY, sep::NAMESPACE),
        FLUID = concatcp!(sep::TYPED, kind::FLUID, sep::NAMESPACE),
        BIOME = concatcp!(sep::TYPED, kind::BIOME, sep::NAMESPACE),
        DIMENSION = concatcp!(sep::TYPED, kind::DIMENSION, sep::NAMESPACE),
    }

    /// Typed values that aren't references to content.
    typed {
        TAG = kind::TAG,
        LANG = "lang",
        IMAGE = "image",
    }

    /// What an image source starts with (`item:apple`).
    image_scheme {
        ITEM = kind::ITEM,
        BLOCK = kind::BLOCK,
        SPRITE = "sprite",
        MOD = "mod",
    }

    /// Yog's files with a fixed name.
    file {
        /// A mod's manifest.
        MANIFEST = "yog.toml",
        /// The versions a mod's build resolved.
        LOCK = "yog.lock",
    }

    /// Yog's programs.
    program {
        /// The command-line tool (`yog build`, `yog schema`, …).
        CLI = "yog",
    }

    /// Where the game keeps resources, and what their files end with.
    res {
        /// The textures' folder: `<ns>:textures/<path>.png`.
        TEXTURES = concatcp!("textures", sep::PATH),
        /// Items' textures and models: `item/apple`.
        ITEM_DIR = concatcp!(kind::ITEM, sep::PATH),
        /// Blocks' textures and models: `block/stone`.
        BLOCK_DIR = concatcp!(kind::BLOCK, sep::PATH),
        /// The GUI atlas's sprites under `textures/` (1.20.2+).
        GUI_SPRITES = concatcp!("gui", sep::PATH, "sprites", sep::PATH),
        PNG = ".png",
        SVG = ".svg",
        JSON = ".json",
        MCMETA = ".mcmeta",
    }

    /// Yog's own ids.
    yog {
        /// An entity's data the hosts keep for the mods.
        STATE = id!(ns::YOG, "state"),
        /// A hanging entity's attachment (its face, size, item).
        HANGING = id!(ns::YOG, "hanging"),
        /// The channel entity data is synced on.
        ENTITY_DATA = id!(ns::YOG, "entity_data"),
        /// The entity data key an entity's own hitbox travels under: width,
        /// height, eye height as three little-endian f32.
        DIMENSIONS = id!(ns::YOG, "dimensions"),
        /// An inventory slot widget's id: `yog:slot:<index>`.
        SLOT_PREFIX = id!(ns::YOG, "slot:"),
        /// The player's inventory widget.
        PLAYER_INVENTORY = id!(ns::YOG, "player_inventory"),
        /// A machine screen's id: `yog:machine:<id>`.
        MACHINE_PREFIX = id!(ns::YOG, "machine:"),
        /// Yog's alias of a game potion: `yog:potion/<name>`.
        POTION_ALIAS = id!(ns::YOG, "potion/"),
        /// The book's pictures.
        BOOK_SHEET = id!(ns::YOG, "gui/book/book"),
        CRAFTING_SHEET = id!(ns::YOG, "gui/book/crafting"),
        /// A screen being designed in the IDE.
        DESIGNER_SCREEN = id!(ns::YOG, "designer"),
        /// A book's default texture (its sheet is [`BOOK_SHEET`]).
        BOOK_TEXTURE = id!(ns::YOG, "gui/book"),
        /// An image source that draws nothing (a bare surface).
        NO_IMAGE = id!(ns::YOG, "none"),
        /// The textures the runtime renders SVG images into: `<prefix><key>_<w>x<h>`.
        DYNAMIC_SVG_PREFIX = id!(ns::YOG, "dynamic/svg_"),
    }

    /// The game's ids Yog's code names.
    game {
        /// The font text is drawn in by default.
        DEFAULT_FONT = id!(ns::MINECRAFT, "default"),
        /// The UI theme drawn like the game's own screens.
        VANILLA_THEME = id!(ns::MINECRAFT, "vanilla"),
        OVERWORLD = id!(ns::MINECRAFT, "overworld"),
        THE_NETHER = id!(ns::MINECRAFT, "the_nether"),
        THE_END = id!(ns::MINECRAFT, "the_end"),
        /// What a book is given to players as.
        WRITTEN_BOOK = id!(ns::MINECRAFT, "written_book"),
    }
}

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

/// The names for the other languages: generated, never edited by hand.
pub mod export {
    use super::ALL;

    const HEADER: &str = "Generated by yog-ids from Yog-Mod-Loader/rust/crates/yog-ids — don't edit; regenerate with `cargo run -p yog-ids --bin yog-ids-export`.";

    /// A TypeScript module: one `as const` object per group.
    pub fn typescript() -> String {
        let mut out = format!("// {HEADER}\n");
        for (group, names) in ALL {
            out.push_str(&format!("\nexport const {} = {{\n", camel(group)));
            for (name, value) in *names {
                out.push_str(&format!("  {name}: {value:?},\n"));
            }
            out.push_str("} as const;\n");
        }
        out.push_str("\n/** Yog's file formats: extension and noun (`Format`). */\nexport const formats = {\n");
        for f in crate::Format::ALL {
            out.push_str(&format!("  {}: {{ ext: {:?}, noun: {:?} }},\n", camel(&format!("{f:?}").to_lowercase()), f.ext(), f.noun()));
        }
        out.push_str("} as const;\n");
        out
    }

    /// A Java class (`package`): a nested class of constants per group.
    pub fn java(package: &str, class: &str) -> String {
        let mut out = format!("// {HEADER}\npackage {package};\n\n/** The names Yog's code shares (yog-ids). */\npublic final class {class} {{\n    private {class}() {{}}\n");
        for (group, names) in ALL {
            out.push_str(&format!("\n    public static final class {} {{\n        private {}() {{}}\n\n", pascal(group), pascal(group)));
            for (name, value) in *names {
                out.push_str(&format!("        public static final String {name} = {value:?};\n"));
            }
            out.push_str("    }\n");
        }
        out.push_str("\n    /** Yog's file formats' extensions (`Format`). */\n    public static final class Ext {\n        private Ext() {}\n\n");
        for f in crate::Format::ALL {
            out.push_str(&format!("        public static final String {} = {:?};\n", format!("{f:?}").to_uppercase(), f.ext()));
        }
        out.push_str("    }\n}\n");
        out
    }

    fn pascal(s: &str) -> String {
        s.split('_').map(|w| w[..1].to_uppercase() + &w[1..]).collect()
    }

    fn camel(s: &str) -> String {
        let p = pascal(s);
        p[..1].to_lowercase() + &p[1..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_composed_from_their_namespace() {
        assert_eq!(yog::STATE, "yog:state");
        assert_eq!(game::OVERWORLD, "minecraft:overworld");
        assert_eq!(image_scheme::ITEM, kind::ITEM);
        assert_eq!(split("stone"), ("minecraft", "stone"));
        assert_eq!(qualify("mymod:x"), "mymod:x");
        assert_eq!(Format::of(".yblock"), Some(Format::Block));
        assert_eq!(Format::Anim.file("walk"), "walk.yanim");
    }

    #[test]
    fn the_exports_name_every_group() {
        let ts = export::typescript();
        assert!(ts.contains("export const imageScheme = {") && ts.contains("  YOG: \"yog\","), "{ts}");
        assert!(ts.contains("  block: { ext: \"yblock\", noun: \"block\" },"), "{ts}");
        let java = export::java("dev.yog.generated", "YogNames");
        assert!(java.contains("public static final class ImageScheme {") && java.contains("public static final String STATE = \"yog:state\";"), "{java}");
    }
}
