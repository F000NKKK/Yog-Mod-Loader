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
        /// A block state's properties: `ns:id[k=v,...]` — what opens and
        /// closes them, separates them, and a property's name from its value.
        STATE_OPEN = "[",
        STATE_CLOSE = "]",
        PROPERTY_SEP = ",",
        PROPERTY_IS = "=",
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
        ITEM = crate::concatcp!(crate::sep::TYPED, crate::kind::ITEM, crate::sep::NAMESPACE),
        BLOCK = crate::concatcp!(crate::sep::TYPED, crate::kind::BLOCK, crate::sep::NAMESPACE),
        ENTITY = crate::concatcp!(crate::sep::TYPED, crate::kind::ENTITY, crate::sep::NAMESPACE),
        RECIPE = crate::concatcp!(crate::sep::TYPED, crate::kind::RECIPE, crate::sep::NAMESPACE),
        ENTRY = crate::concatcp!(crate::sep::TYPED, crate::kind::ENTRY, crate::sep::NAMESPACE),
        CATEGORY = crate::concatcp!(crate::sep::TYPED, crate::kind::CATEGORY, crate::sep::NAMESPACE),
        FLUID = crate::concatcp!(crate::sep::TYPED, crate::kind::FLUID, crate::sep::NAMESPACE),
        BIOME = crate::concatcp!(crate::sep::TYPED, crate::kind::BIOME, crate::sep::NAMESPACE),
        DIMENSION = crate::concatcp!(crate::sep::TYPED, crate::kind::DIMENSION, crate::sep::NAMESPACE),
        TEXTURE = crate::concatcp!(crate::sep::TYPED, crate::kind::TEXTURE, crate::sep::NAMESPACE),
        MATERIAL = crate::concatcp!(crate::sep::TYPED, crate::kind::MATERIAL, crate::sep::NAMESPACE),
        /// A tag, its registry after: `@tag:item:c:ingots`.
        TAG = crate::concatcp!(crate::sep::TYPED, crate::kind::TAG, crate::sep::NAMESPACE),
    }

    /// Typed values that aren't references to content.
    typed {
        TAG = crate::kind::TAG,
        LANG = "lang",
        IMAGE = "image",
        /// Bytes as they are, base64 in text: `@bytes:iVBORw0…`.
        BYTES = "bytes",
    }
}
