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
        ARMOR = "armor",
        BASE = "base",
        BLOCK = "block",
        CATEGORY = "category",
        CONTENT = "content",
        DAMAGE = "damage",
        DIMENSION = "dimension",
        EQUIPMENT = "equipment",
        EVENT = "event",
        FUNCTION = "function",
        GENERIC = "generic",
        GUI = "gui",
        INT = "int",
        ITEM = "item",
        KEY = "key",
        MODELS = "models",
        MODS = "mods",
        OPTIONAL = "optional",
        PLAYER = "player",
        RESULT = "result",
        SPRITE = "sprite",
        STATE = "state",
        TAG = "tag",
        TEXTURE = "texture",
        UI = "ui",
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
        SOUND_GROUP = crate::concatcp!(crate::word::SOUND, crate::sep::WORD, crate::recipe_field::GROUP),
        RECIPE_KIND = crate::concatcp!(RECIPE, crate::sep::WORD, crate::word::KIND),
        TOOL_MATERIAL = crate::concatcp!("tool", crate::sep::WORD, MATERIAL),
        ARMOR_MATERIAL = crate::concatcp!(crate::word::ARMOR, crate::sep::WORD, MATERIAL),
        ORE = "ore",
        TABLE = "table",
        TAG = crate::word::TAG,
        DAMAGE_TYPE = crate::concatcp!(crate::word::DAMAGE, crate::sep::WORD, crate::word::TYPE),
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

    /// What a problem is, as the Error List and the docs name it: `YD…` a
    /// problem with Yog Data, `YC…` with a request to the CLI.
    problem_code {
        /// Any other problem with a value.
        INVALID = "YD0000",
        /// Text that doesn't parse.
        SYNTAX = "YD0001",
        /// A value of another type than its field's.
        EXPECTED = "YD0002",
        /// A bare id where a reference goes (`@item:…`).
        NOT_A_REFERENCE = "YD0003",
        /// A reference to another kind of content than its field takes.
        WRONG_KIND = "YD0004",
        UNKNOWN_FIELD = "YD0005",
        MISSING_FIELD = "YD0006",
        OUT_OF_RANGE = "YD0007",
        /// A string written as a typed value that isn't one (`@Item:x`).
        BAD_TYPED = "YD0008",
        /// Binary Yog Data that won't decode.
        BAD_BINARY = "YD0009",
        /// A request that failed otherwise.
        FAILED = "YC0000",
        /// What a request names isn't there.
        NOT_FOUND = "YC0001",
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
