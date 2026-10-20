//! The words every name is made of: namespaces, separators, kinds of
//! content, typed-value words.

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
        /// What separates the fields of a host's reply to the runtime.
        FIELD = "\t",
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
        FONT = "font",
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
        TEXTURE = "texture",
        MATERIAL = "material",
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
        TEXTURE = concatcp!(sep::TYPED, kind::TEXTURE, sep::NAMESPACE),
        MATERIAL = concatcp!(sep::TYPED, kind::MATERIAL, sep::NAMESPACE),
    }

    /// Typed values that aren't references to content.
    typed {
        TAG = kind::TAG,
        LANG = "lang",
        IMAGE = "image",
        /// Bytes as they are, base64 in text: `@bytes:iVBORw0…`.
        BYTES = "bytes",
    }
}
