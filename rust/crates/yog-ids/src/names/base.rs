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
        /// A name's words: `sound_group`.
        WORD = "_",
        /// A file's name from its extension: `yog.toml`.
        EXT = ".",
        /// The words of a folder's name: `yog-mods`.
        HYPHEN = "-",
        /// A `.yui` element's member: `Grid.Row`, `Screen.Resources`.
        MEMBER = crate::sep::EXT,
        /// What starts a typed value: `@item:minecraft:stick`.
        TYPED = "@",
        /// What marks a tag among ids: `#minecraft:logs`.
        TAG = "#",
        /// What marks a tag where an ingredient is shown: `#c:dusts`.
        TAG_SHOWN = crate::sep::TAG,
        /// A block state's properties: `ns:id[k=v,...]` — what opens and
        /// closes them, separates them, and a property's name from its value.
        STATE_OPEN = "[",
        STATE_CLOSE = "]",
        PROPERTY_SEP = ",",
        PROPERTY_IS = "=",
    }

    /// The game's six directions, in its order: a block's faces, the way
    /// it faces, the sides it connects on.
    direction {
        DOWN = "down",
        UP = "up",
        NORTH = "north",
        SOUTH = "south",
        WEST = "west",
        EAST = "east",
    }

    /// Words other names are made of, said once: sides and places, yes and no,
    /// the common fields' names.
    word {
        BASE = "base",
        BLOCK = "block",
        CATEGORY = "category",
        CONTENT = "content",
        DIMENSION = "dimension",
        EVENT = "event",
        FUNCTION = "function",
        GUI = "gui",
        INT = "int",
        ITEM = "item",
        KEY = "key",
        OPTIONAL = "optional",
        RESULT = "result",
        SPRITE = "sprite",
        TAG = "tag",
        TEXTURE = "texture",
        VALUES = "values",
        /// The plurals the game names folders and atlases by.
        ITEMS = "items",
        BLOCKS = "blocks",
        ENTITIES = "entities",
        RECIPES = "recipes",
        LOOT = "loot",
        SPAWN_EGG = "spawn_egg",
        SOUND = "sound",
        ANIMATION = "animation",
        ANY = "any",
        AUTO = "auto",
        BOOL = "bool",
        BOTTOM = "bottom",
        BYTES = "bytes",
        CENTER = "center",
        COLOR = "color",
        COUNT = "count",
        DATA = "data",
        DEFAULT = "default",
        END = "end",
        ENTRIES = "entries",
        FALSE = "false",
        FILE = "file",
        FILL = "fill",
        HEIGHT = "height",
        ID = "id",
        IMAGE = "image",
        KIND = "kind",
        LEFT = "left",
        MOD = "mod",
        MODEL = "model",
        NAME = "name",
        NONE = "none",
        NUMBER = "number",
        OUTLINE = "outline",
        RIGHT = "right",
        SIZE = "size",
        SOURCE = "source",
        SRC = "src",
        START = "start",
        STRETCH = "stretch",
        STRING = "string",
        TEXT = "text",
        TOP = "top",
        TRUE = "true",
        TYPE = "type",
        UV = "uv",
        VALUE = "value",
        WHEN = "when",
        WIDTH = "width",
        X = "x",
        Y = "y",
    }

    /// The kinds of content: what a reference names (`@item:…`), what a
    /// `.ydef` defines (`kind: "ladder"`), what a mod registers with the
    /// host (`content("potion", …)`).
    kind {
        ITEM = crate::word::ITEM,
        BLOCK = crate::word::BLOCK,
        ENTITY = "entity",
        RECIPE = "recipe",
        ENTRY = "entry",
        CATEGORY = crate::word::CATEGORY,
        BOOK = "book",
        FLUID = "fluid",
        BIOME = "biome",
        BIOME_FEATURE = crate::concatcp!(BIOME, crate::sep::WORD, "feature"),
        DIMENSION = crate::word::DIMENSION,
        FONT = "font",
        ATTRIBUTE = "attribute",
        EFFECT = "effect",
        POTION = "potion",
        INVENTORY = "inventory",
        STARTUP_GRANT = "startup_grant",
        ADVANCEMENT = "advancement",
        LADDER = "ladder",
        LEVEL = "level",
        SOUND_GROUP = crate::concatcp!(crate::word::SOUND, crate::sep::WORD, "group"),
        RECIPE_KIND = crate::concatcp!(RECIPE, crate::sep::WORD, crate::word::KIND),
        TOOL_MATERIAL = crate::concatcp!("tool", crate::sep::WORD, MATERIAL),
        ARMOR_MATERIAL = crate::concatcp!("armor", crate::sep::WORD, MATERIAL),
        ORE = "ore",
        TABLE = "table",
        TAG = crate::word::TAG,
        DAMAGE_TYPE = crate::concatcp!("damage", crate::sep::WORD, crate::word::TYPE),
        TREE = "tree",
        LOOT_MODIFIER = crate::concatcp!(crate::word::LOOT, crate::sep::WORD, "modifier"),
        LOOT_TABLE = crate::concatcp!(crate::word::LOOT, crate::sep::WORD, TABLE),
        TEXTURE = crate::word::TEXTURE,
        MATERIAL = "material",
    }

    /// What a reference to content of a kind starts with: `@item:`.
    reference {
        ITEM = crate::concatcp!(crate::sep::TYPED, crate::kind::ITEM, crate::sep::NAMESPACE),
        BLOCK = crate::concatcp!(crate::sep::TYPED, crate::kind::BLOCK, crate::sep::NAMESPACE),
        CATEGORY = crate::concatcp!(crate::sep::TYPED, crate::kind::CATEGORY, crate::sep::NAMESPACE),
        TEXTURE = crate::concatcp!(crate::sep::TYPED, crate::kind::TEXTURE, crate::sep::NAMESPACE),
        /// A tag, its registry after: `@tag:item:c:ingots`.
        TAG = crate::concatcp!(crate::sep::TYPED, crate::kind::TAG, crate::sep::NAMESPACE),
    }

    /// Typed values that aren't references to content.
    typed {
        TAG = crate::kind::TAG,
        LANG = "lang",
        IMAGE = crate::word::IMAGE,
        /// Bytes as they are, base64 in text: `@bytes:iVBORw0…`.
        BYTES = crate::word::BYTES,
    }
}
