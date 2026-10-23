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
        /// A dot: a file's name from its extension (`yog.toml`), a `.yui`
        /// element's member (`Grid.Row`), a key's or a sound event's parts
        /// (`casting.cast`).
        DOT = ".",
        /// The words of a folder's name: `yog-mods`.
        HYPHEN = "-",
        /// What starts a typed value: `@item:minecraft:stick`.
        TYPED = "@",
        /// What marks a tag among ids: `#minecraft:logs`.
        TAG = "#",
        /// What marks a tag where an ingredient is shown: `#c:dusts`.
        TAG_SHOWN = crate::sep::TAG,
        /// A block state's properties: `ns:id[k=v,...]` — what opens and
        /// closes them, separates them, and a property's name from its value.
        /// What separates the items of a list written in one string.
        LIST = ",",
        STATE_OPEN = "[",
        STATE_CLOSE = "]",
        PROPERTY_SEP = crate::sep::LIST,
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
        DISPLAY = "display",
        MESSAGE = "message",
        PARTICLE = "particle",
        TARGET = "target",
        WALL = "wall",
        ARMOR = "armor",
        BASE = "base",
        BLOCK = "block",
        AIR = "air",
        CATEGORY = "category",
        CHUNK = "chunk",
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
        PNG = "png",
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
        DOC = "doc",
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
        SOUND = crate::word::SOUND,
        PARTICLE = crate::word::PARTICLE,
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

    /// What's wrong with Yog Data (`YD…`), as the Error List and the docs name it.
    data_problem {
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
    }

    /// What's wrong with a request to the CLI's service (`YC…`).
    request_problem {
        /// A request that failed otherwise.
        FAILED = "YC0000",
        /// What a request names isn't there.
        NOT_FOUND = "YC0001",
    }

    /// What's wrong with a mod's build (`YB…`).
    build_problem {
        /// A build that failed otherwise.
        FAILED = "YB0000",
        /// `yog.toml` that isn't right.
        MANIFEST = "YB0001",
        /// A content file (`content/**`) that isn't right.
        CONTENT = "YB0002",
        /// A resource file (`.yres`) that isn't right.
        RESOURCES = "YB0003",
        /// A screen (`.yui`) that isn't right.
        SCREEN = "YB0004",
        /// Cargo itself failed — resolving, fetching, its toolchain — not the compiler.
        CARGO = "YB0005",
        /// The compiler's say without a code of its own.
        COMPILER = "YB0006",
        /// No platform's library got built.
        NO_PLATFORM = "YB0007",
        /// The `.yog` couldn't be packaged.
        PACKAGE = "YB0008",
        /// A benchmark file (`benches/*.rs`) that isn't right.
        BENCH = "YB0009",
    }

    /// What's wrong with Yog Script (`.ys`, `.yns`: `YS…`).
    script_problem {
        /// A character no token starts with.
        UNEXPECTED_CHAR = "YS0001",
        /// A string or comment that never ends.
        UNTERMINATED = "YS0002",
        /// A number that doesn't read (`0x`, `1e`).
        BAD_NUMBER = "YS0003",
        /// Something else than the grammar allows there.
        EXPECTED = "YS0004",
        /// A patch outside a `.yns` file.
        PATCH_OUTSIDE_NANITES = "YS0005",
        /// A content reference that isn't one (`@item`, `@:x`).
        BAD_REFERENCE = "YS0006",
    }

    /// A mod's stores (`yog-storage`): `<root>/yog-data/<mod>/…`, the
    /// whole-game and whole-world stores by name, the rest in folders.
    storage {
        DIR = "yog-data",
        GAME = crate::bus_word::GAME,
        WORLD = crate::bus_word::WORLD,
        DIMENSION = crate::word::DIMENSION,
        PLAYER = crate::word::PLAYER,
        ENTITY = crate::kind::ENTITY,
        CHUNK = crate::word::CHUNK,
        /// What a name's separators become in a store's file name, and
        /// what parts a chunk store's name (`<dim>_<cx>_<cz>`).
        SEP = crate::sep::WORD,
    }

    /// The words of Yog Data's `#[data(...)]` attribute: the attribute
    /// itself and each option (the derive reads them; `#[yog_export]`
    /// writes them).
    data_attr {
        CRATE = "crate",
        DATA = crate::word::DATA,
        DEFAULT = crate::word::DEFAULT,
        DOC = crate::word::DOC,
        OTHER = "other",
        RANGE = "range",
        RENAME = "rename",
        RENAME_ALL = "rename_all",
        /// `rename_all`'s one case: fields (a struct's) or variants (an enum's) in lower camel case.
        CAMEL_CASE = "camelCase",
        SKIP = "skip",
        TAG = crate::word::TAG,
        VIEW = "view",
        WITH = "with",
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
