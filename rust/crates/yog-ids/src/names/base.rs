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
        WRITE = "write",
        REMOVE = "remove",
        SESSION = "session",
        STORAGE = "storage",
        VIDEO = "video",
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
        FLOAT = "float",
        FRAME = "frame",
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
        CUTSCENE = "cutscene",
        DATA = "data",
        DEFAULT = "default",
        DOC = "doc",
        DYNAMIC = "dynamic",
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

    /// The types a command's arguments are (its schema's words: `int
    /// player`), as the hosts read them with the game's parsers (a block
    /// position is written a list, `x,y,z`).
    command_arg {
        INT = crate::word::INT,
        FLOAT = crate::word::FLOAT,
        WORD = "word",
        /// The rest of the line: last only.
        STRING = crate::word::STRING,
        PLAYER = crate::word::PLAYER,
        BLOCKPOS = "blockpos",
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
        VIDEO = crate::word::VIDEO,
    }

    /// What a reference to content of a kind starts with: `@item:`.
    reference {
        ITEM = crate::concatcp!(crate::sep::TYPED, crate::kind::ITEM, crate::sep::NAMESPACE),
        BLOCK = crate::concatcp!(crate::sep::TYPED, crate::kind::BLOCK, crate::sep::NAMESPACE),
        CATEGORY = crate::concatcp!(crate::sep::TYPED, crate::kind::CATEGORY, crate::sep::NAMESPACE),
        TEXTURE = crate::concatcp!(crate::sep::TYPED, crate::kind::TEXTURE, crate::sep::NAMESPACE),
        VIDEO = crate::concatcp!(crate::sep::TYPED, crate::kind::VIDEO, crate::sep::NAMESPACE),
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

    /// The variables Yog hands Yog Script's preprocessor (`#if MC >= 1.21`).
    script_define {
        /// The game version a `.yns` is being compiled for.
        GAME = "MC",
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
        /// A name declared twice in one place.
        DUPLICATE = "YS0007",
        /// `break` or `continue` outside a loop.
        OUTSIDE_LOOP = "YS0008",
        /// `skip` outside a patch's `before`.
        SKIP_OUTSIDE_BEFORE = "YS0009",
        /// `it` outside a block lambda (`{ it * 2 }`).
        IT_OUTSIDE_LAMBDA = "YS0010",
        /// `this` outside a type's, a content's, a component's or a patch's members.
        THIS_OUTSIDE_MEMBER = "YS0011",
        /// What can't be assigned (`const`, `readonly`, a method, a type) assigned to.
        ASSIGN_TO_CONSTANT = "YS0012",
        /// A name not in C#'s case for what it names (a warning).
        NAMING = "YS0013",
        /// `await` outside an `async` method.
        AWAIT_OUTSIDE_ASYNC = "YS0014",
        /// A value of one type where another is wanted.
        TYPE_MISMATCH = "YS0015",
        /// `null` where the type can't be `null`, or a nullable used without `?.`.
        NULLABLE = "YS0016",
        /// A member the type doesn't have.
        UNKNOWN_MEMBER = "YS0017",
        /// A class of one side reached from code of the other.
        WRONG_SIDE = "YS0018",
        /// A call with arguments the method doesn't take.
        WRONG_ARGS = "YS0019",
        /// A `.yns` without `targets mc …;`, or `targets` outside a `.yns`.
        TARGETS = "YS0020",
        /// `targets` naming no version Yog runs on.
        NO_VERSION = "YS0021",
        /// A name two `using`s bring in (C#'s CS0104).
        AMBIGUOUS = "YS0022",
        /// A preprocessor directive out of place: an `#elif`, `#else` or
        /// `#endif` without its `#if`, an `#if` never ended, one in a `.ys`.
        DIRECTIVE = "YS0023",
        /// A lifetime (`'a`) no generic list declares.
        LIFETIME = "YS0024",
        /// A value used after `move` handed it on (Rust's E0382).
        MOVED = "YS0025",
        /// A borrow that breaks the rules: a `ref` beside another borrow,
        /// a change or a move of what's borrowed, one outliving its owner.
        BORROW = "YS0026",
        /// An argument passed otherwise than its parameter takes it
        /// (`ref` for `in`, none for `out`, …).
        PASS = "YS0027",
        /// Inheritance that breaks C#'s rules: two base classes, a sealed
        /// one inherited, an abstract member left unimplemented, an
        /// `override` of nothing, an abstract class made with `new`.
        INHERIT = "YS0028",
        /// A `switch` over an enum leaving a case out, with no `_`.
        NOT_EXHAUSTIVE = "YS0029",
        /// A macro declared or called wrong: a rule, a fragment's kind, a
        /// call no rule matches, an expansion without end.
        MACRO = "YS0030",
        /// `[Name]` naming something that isn't an attribute class.
        ATTRIBUTE = "YS0031",
        /// What the compiler doesn't write as Java yet.
        NOT_COMPILED = "YS0032",
        /// Sending to the server from a mod whose `yog.toml` doesn't say so
        /// (`[network] to_server = true`).
        UNDECLARED = "YS0033",
        /// An operator declared as C# doesn't allow: not `public static`, of
        /// none of its own type, or one of a pair without the other (`==`
        /// and `!=`, `<` and `>`, `<=` and `>=`).
        OPERATOR = "YS0034",
        /// A type name nothing declares where it's written: no type of the
        /// code's namespace nor of one a `using` brings in (C#'s CS0246).
        UNKNOWN_TYPE = "YS0035",
        /// A local read before it's given a value, or an `out` parameter
        /// not given one on a path out of its method (C#'s CS0165, CS0177).
        UNASSIGNED = "YS0036",
        /// A property in a content declaration (`block Lamp { Hardness = 2; }`):
        /// they're its file's (`lamp.yblock`), the declaration only its behaviour.
        CONTENT_PROPERTY = "YS0037",
        /// A content declaration without its file (`block Lamp` without a
        /// `lamp.yblock` under `content/`).
        NO_CONTENT_FILE = "YS0038",
        /// `yield` outside an iterator (a method giving an `IEnumerable<T>`,
        /// `IEnumerator<T>` or `IAsyncEnumerable<T>`), or `return value` in one.
        ITERATOR = "YS0039",
        /// A value crossing threads that can't (`ys-concurrency.md` §2): not
        /// sendable where it moves to another thread, not shareable where
        /// several reach it at once.
        THREADS = "YS0040",
        /// `unsafe { … }`: the threads checks skipped inside (a warning).
        UNSAFE = "YS0041",
        /// A type given for a type parameter that doesn't meet its `where`
        /// constraints, or `new T()` without `new()` (C#'s CS0311, CS0304).
        CONSTRAINT = "YS0042",
        /// A `[LoggerMessage]` method written so nothing can be generated:
        /// not `partial`, no logger, no level, a hole no parameter fills.
        LOGGER_MESSAGE = "YS0043",
        /// A log written through `LogInformation` & co. (a warning, .NET's
        /// CA1848): a `[LoggerMessage]` method writes it faster.
        LOG_BY_METHOD = "YS0044",
        /// A name nothing declares where it's read (C#'s CS0103).
        UNKNOWN_NAME = "YS0045",
        /// A `goto` to a label not in its block nor one around it, a
        /// label declared twice, `goto case`/`goto default` outside a `switch`.
        GOTO = "YS0046",
        /// A `new` leaving a `required` member unset (C#'s CS9035).
        REQUIRED = "YS0047",
        /// A `saved` or `synced` member of a type Yog Data doesn't keep
        /// (storage.md S3).
        STATE_TYPE = "YS0048",
        /// A `saved` or `synced` member of a type with no owner scope (a
        /// plain class): state needs an owner.
        STATE_OWNER = "YS0049",
        /// A `was` on a member that isn't `saved`, or naming a type the
        /// member can't be read from (storage.md S5).
        STATE_WAS = "YS0050",
    }

    /// A mod's stores (`yog-storage`): `<root>/yog-data/<mod>/…`, the
    /// whole-game and whole-world stores by name, the rest in folders.
    /// A mod's Yog Script on the JVM: its classes' package starts with
    /// this, then the mod's id (`yogscript.ystest`) — how the runtime
    /// tells whose code calls it.
    script_jvm {
        PACKAGE_ROOT = "yogscript",
    }

    storage {
        DIR = "yog-data",
        GAME = crate::bus_word::GAME,
        WORLD = crate::bus_word::WORLD,
        DIMENSION = crate::word::DIMENSION,
        PLAYER = crate::word::PLAYER,
        ENTITY = crate::kind::ENTITY,
        CHUNK = crate::word::CHUNK,
        /// The store kept while the game runs, never written.
        SESSION = crate::word::SESSION,
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
