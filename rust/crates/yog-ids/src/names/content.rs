//! Resources, files and content: where things live and what they're called.

names! {

    /// A `.yres` entry's type (`{ "type": "sound", ... }`).
    res_kind {
        STRING = crate::word::STRING,
        IMAGE = crate::typed::IMAGE,
        FILE = crate::word::FILE,
        SOUND = crate::word::SOUND,
        COLOR = crate::word::COLOR,
        NUMBER = crate::word::NUMBER,
        BOOL = crate::word::BOOL,
        VIDEO = crate::word::VIDEO,
    }

    /// What an image source starts with (`item:apple`).
    image_scheme {
        ITEM = crate::kind::ITEM,
        BLOCK = crate::kind::BLOCK,
        SPRITE = crate::word::SPRITE,
        MOD = crate::word::MOD,
        VIDEO = crate::kind::VIDEO,
    }

    /// Yog's files with a fixed name.
    file {
        /// A mod's manifest.
        MANIFEST = crate::concatcp!(crate::ns::YOG, crate::sep::DOT, "toml"),
        /// The versions a mod's build resolved.
        LOCK = crate::concatcp!(crate::ns::YOG, crate::sep::DOT, "lock"),
    }

    /// What a built `.yog` holds besides the mod's files.
    archive {
        /// What it says about its mod (`yog_formats::manifest::Built`, Yog Data).
        MANIFEST = crate::concatcp!(crate::word::MOD, crate::sep::DOT, crate::Format::Data.ext()),
        /// Its native libraries, one folder per platform: `natives/linux-x86_64/libmymod.so`.
        NATIVES = "natives",
    }

    /// A picture of few colors as a `.yog` keeps it (`yog_media::yimage`):
    /// under its `.png` name, told apart by these first bytes; the host
    /// hands the game its PNG, made in memory.
    yimage {
        MAGIC = "YIMG",
    }

    /// A release's debug info, split from its natives: one folder per
    /// platform (`symbols/linux-x86_64/libmymod.so.debug`), read beside the
    /// stripped native or from Yog Installer's symbols component.
    symbols {
        DIR = "symbols",
        /// What a native's split debug info adds to its file name.
        EXT = "debug",
    }

    /// The pack the build writes for the hosts.
    pack {
        /// The game's data: `data/<ns>/…`.
        DATA = crate::word::DATA,
        /// The game's assets: `assets/<ns>/…`.
        ASSETS = "assets",
        /// Tags, under a namespace's data: `data/<ns>/tags/<registry>/<path>.json`.
        TAGS = "tags",
        /// Worldgen, under a namespace's data: `data/<ns>/worldgen/<registry>/<path>.json`.
        WORLDGEN = "worldgen",
        /// Overlays for one loader and version: `when/<loader>/<version>/<file>`.
        OVERLAYS = crate::word::WHEN,
        /// An overlay's loader when it's for every loader.
        ANY_LOADER = crate::word::ANY,
        /// An overlay's version when it's for every version.
        ANY_VERSION = crate::word::ANY,
    }

    /// A mod project's folders.
    project {
        /// Its resources: `.yres` sets, `.ytexture`s, ….
        RESOURCES = "resources",
        /// Its content: blocks, items, recipes, books, definitions.
        CONTENT = crate::word::CONTENT,
        /// Where a copy of a game font goes, under its content.
        FONT_COPIES = crate::kind::FONT,
        /// Its code and screens.
        SRC = crate::word::SRC,
        /// Where `yog build` writes: the generated crate, generated code, listings.
        BUILD = crate::concatcp!(crate::sep::DOT, crate::ns::YOG, crate::sep::HYPHEN, "build"),
    }

    /// A game's folders Yog uses.
    game_dir {
        /// The `.yog` mods the game loads: `<game>/yog-mods`.
        MODS = crate::concatcp!(crate::ns::YOG, crate::sep::HYPHEN, crate::word::MODS),
    }


    /// Yog's programs.
    program {
        /// The command-line tool (`yog build`, `yog schema`, …).
        CLI = crate::ns::YOG,
        /// The CLI's service command: tools read and write every format through it.
        SERVE = "serve",
        /// Yog Script's tool (`yog-script check`, `yog-script index`), installed
        /// as its own component; the CLI compiles a mod's scripts through
        /// `yog-script-build` itself.
        SCRIPT = "yog-script",
    }

    /// The structs the generated code names a mod's content by
    /// (`Items::RUBY`); a `.yres` set of the same name holds that content's
    /// names (`Items.yres`: `item.<mod>.<name>`).
    code_set {
        BLOCKS = "Blocks",
        ITEMS = "Items",
        ENTITIES = "Entities",
        ADVANCEMENTS = "Advancements",
        MODELS = "Models",
        ANIMS = "Anims",
        TEXTURES = "Textures",
    }

    /// Yog's crates a mod's code builds with, as Cargo names them (Rust's
    /// name has `_` for `-`: [`crate::rust_name`]).
    rust_crate {
        /// What every one of Yog's crates is named with.
        PREFIX = crate::concatcp!(crate::ns::YOG, crate::sep::HYPHEN),
        /// The API crate: its word, its name, its name in Rust code, and the
        /// `[dependencies]` key that pins its version alone.
        API_WORD = "api",
        API = crate::concatcp!(PREFIX, API_WORD),
        API_RUST_NAME = crate::concatcp!(crate::ns::YOG, crate::sep::WORD, API_WORD),
        API_VERSION_KEY = crate::concatcp!(API_RUST_NAME, crate::sep::WORD, crate::serve_method::VERSION),
        ABI = crate::concatcp!(PREFIX, "abi"),
        UI = crate::concatcp!(PREFIX, crate::word::UI),
        KIT = crate::concatcp!(PREFIX, "kit"),
        GFX = crate::concatcp!(PREFIX, "gfx"),
        REGISTRY = crate::concatcp!(PREFIX, crate::serve_param::REGISTRY),
        /// The data crate: its name, and its name in Rust code (yog-api's module of it too).
        DATA = crate::concatcp!(PREFIX, crate::word::DATA),
        DATA_RUST_NAME = crate::concatcp!(crate::ns::YOG, crate::sep::WORD, crate::word::DATA),
    }

    /// Where the game keeps resources, and what their files end with.
    res {
        /// The textures' folder: `<ns>:textures/<path>.png`.
        TEXTURES = crate::concatcp!(crate::model::TEXTURES, crate::sep::PATH),
        /// The models' folder: `<ns>:models/<path>.json`.
        MODELS = crate::concatcp!(crate::word::MODELS, crate::sep::PATH),
        /// The block states' folder: `<ns>:blockstates/<path>.json`.
        BLOCKSTATES = crate::concatcp!("blockstates", crate::sep::PATH),
        /// Items' textures and models: `item/apple`.
        ITEM_DIR = crate::concatcp!(crate::kind::ITEM, crate::sep::PATH),
        /// Item definitions (1.21.4+): `<ns>:items/<path>.json`.
        ITEM_DEFINITIONS = crate::concatcp!(crate::word::ITEMS, crate::sep::PATH),
        /// Equipment's worn looks (1.21.4+): `<ns>:equipment/<path>.json`.
        EQUIPMENT = crate::concatcp!(crate::word::EQUIPMENT, crate::sep::PATH),
        /// Worn equipment's textures (1.21.4+): `entity/equipment/<layer>/<path>`.
        EQUIPMENT_TEXTURES = crate::concatcp!(crate::kind::ENTITY, crate::sep::PATH, crate::word::EQUIPMENT, crate::sep::PATH),
        /// Armor's worn textures before 1.21.4: `models/armor/<path>_layer_<n>`.
        ARMOR_TEXTURES = crate::concatcp!(crate::word::MODELS, crate::sep::PATH, crate::word::ARMOR, crate::sep::PATH),
        /// Between an armor texture's name and its layer's number.
        ARMOR_LAYER = "_layer_",
        /// What a spawn egg's item path ends with: `<mob>_spawn_egg`.
        SPAWN_EGG_SUFFIX = crate::concatcp!(crate::sep::WORD, crate::word::SPAWN_EGG),
        /// Blocks' textures and models: `block/stone`.
        BLOCK_DIR = crate::concatcp!(crate::kind::BLOCK, crate::sep::PATH),
        /// The GUI atlas's sprites under `textures/` (1.20.2+).
        GUI_SPRITES = crate::concatcp!(crate::word::GUI, crate::sep::PATH, "sprites", crate::sep::PATH),
        /// The atlases' definitions: `<ns>:atlases/<atlas>.json` (every pack's sources combine).
        ATLASES = crate::concatcp!("atlases", crate::sep::PATH),
        /// The language files' folder: `<ns>:lang/<locale>.json`.
        LANG = crate::concatcp!(crate::typed::LANG, crate::sep::PATH),
        /// Models the game draws in code (`builtin/entity`), not from elements.
        BUILTIN = crate::concatcp!("builtin", crate::sep::PATH),
        /// Sounds: `<ns>:sounds/<path>.ogg`, their events in `<ns>:sounds.json`.
        SOUNDS = crate::concatcp!(SOUNDS_WORD, crate::sep::PATH),
        SOUNDS_JSON = crate::concatcp!(SOUNDS_WORD, JSON),
        SOUNDS_WORD = "sounds",
        /// Particles' pictures: `<ns>:particles/<name>.json`.
        PARTICLES = crate::concatcp!("particles", crate::sep::PATH),
        OGG = ".ogg",
        PNG = crate::concatcp!(crate::sep::DOT, crate::word::PNG),
        SVG = ".svg",
        JSON = ".json",
        MCMETA = ".mcmeta",
    }

    /// Yog's own ids.
    yog {
        /// An entity's data the hosts keep for the mods.
        STATE = crate::id!(crate::ns::YOG, crate::word::STATE),
        /// A hanging entity's attachment (its face, size, item).
        HANGING = crate::id!(crate::ns::YOG, "hanging"),
        /// The key an entity's data is saved under in its NBT.
        ENTITY_NBT = crate::id!(crate::ns::YOG, crate::word::DATA),
        /// The channel entity data is synced on.
        ENTITY_DATA = crate::id!(crate::ns::YOG, crate::concatcp!(crate::kind::ENTITY, crate::sep::WORD, crate::word::DATA)),
        /// The entity data key an entity's own hitbox travels under: width,
        /// height, eye height as three little-endian f32.
        DIMENSIONS = crate::id!(crate::ns::YOG, "dimensions"),
        /// An inventory slot widget's id: `yog:slot:<index>`.
        SLOT_PREFIX = crate::id!(crate::ns::YOG, "slot:"),
        /// The player's inventory widget.
        PLAYER_INVENTORY = crate::id!(crate::ns::YOG, crate::concatcp!(crate::word::PLAYER, crate::sep::WORD, crate::kind::INVENTORY)),
        /// A machine screen's id: `yog:machine:<id>`.
        MACHINE_PREFIX = crate::id!(crate::ns::YOG, "machine:"),
        /// Yog's alias of a game potion: `yog:potion/<name>`.
        POTION_ALIAS = crate::id!(crate::ns::YOG, "potion/"),
        /// Yog's alias of a game font, `yog:font/<name>` — the only way
        /// anything names one of the game's fonts.
        FONT_ALIAS = crate::id!(crate::ns::YOG, "font/"),
        /// The font text is drawn in by default (the game's).
        DEFAULT_FONT = crate::concatcp!(FONT_ALIAS, crate::game_font::DEFAULT),
        /// The model every mod's spawn egg inherits: Yog's egg in two layers, base and spots.
        SPAWN_EGG_MODEL = crate::id!(crate::ns::YOG, crate::concatcp!(crate::res::ITEM_DIR, crate::word::SPAWN_EGG)),
        /// The book's pictures.
        BOOK_SHEET = crate::concatcp!(BOOK_TEXTURE, crate::sep::PATH, crate::kind::BOOK),
        CRAFTING_SHEET = crate::concatcp!(BOOK_TEXTURE, crate::sep::PATH, "crafting"),
        /// A screen being designed in the IDE.
        DESIGNER_SCREEN = crate::id!(crate::ns::YOG, "designer"),
        /// The screen a cutscene plays in (the runtime's own).
        CUTSCENE_SCREEN = crate::id!(crate::ns::YOG, crate::word::CUTSCENE),
        /// A book's default texture (its sheet is [`BOOK_SHEET`]).
        BOOK_TEXTURE = crate::id!(crate::ns::YOG, crate::concatcp!(crate::word::GUI, crate::sep::PATH, crate::kind::BOOK)),
        /// The textures the runtime renders SVG images into: `<prefix><key>_<w>x<h>`.
        DYNAMIC_SVG_PREFIX = crate::id!(crate::ns::YOG, crate::concatcp!(crate::word::DYNAMIC, crate::sep::PATH, "svg", crate::sep::WORD)),
        /// The textures the runtime shows videos' frames in:
        /// `<prefix><player>.png`, a texture's full id (an image source as it is).
        DYNAMIC_VIDEO_PREFIX = crate::id!(crate::ns::YOG, crate::concatcp!(crate::res::TEXTURES, crate::word::DYNAMIC, crate::sep::PATH, crate::word::VIDEO, crate::sep::WORD)),
        /// The textures the runtime draws text in Yog's fonts into: `<prefix><n>`.
        DYNAMIC_TEXT_PREFIX = crate::id!(crate::ns::YOG, crate::concatcp!(crate::word::DYNAMIC, crate::sep::PATH, crate::word::TEXT, crate::sep::WORD)),
    }

    /// The game's fonts, by name: Yog aliases each as
    /// `yog:font/<name>` ([`crate::yog::FONT_ALIAS`]).
    game_font {
        DEFAULT = crate::word::DEFAULT,
        /// Unifont.
        UNIFORM = "uniform",
        /// The enchanting table's.
        ALT = "alt",
        ILLAGERALT = "illageralt",
    }

    /// The game's attributes by name — the hosts find each on every
    /// version (`max_health`, `generic.max_health` before 1.21.2).
    attribute {
        MAX_HEALTH = "max_health",
    }

    /// The game's modes a player is put in.
    gamemode {
        SURVIVAL = "survival",
        CREATIVE = "creative",
        ADVENTURE = "adventure",
        SPECTATOR = "spectator",
    }

    /// The game's ids Yog's code names.
    game {
        /// The game's own id of its default font (hosts only; everything
        /// else says [`crate::yog::DEFAULT_FONT`]).
        DEFAULT_FONT = crate::id!(crate::ns::MINECRAFT, crate::game_font::DEFAULT),
        /// The UI theme drawn like the game's own screens.
        VANILLA_THEME = crate::id!(crate::ns::MINECRAFT, "vanilla"),
        OVERWORLD = crate::id!(crate::ns::MINECRAFT, "overworld"),
        /// The empty block: what's left where a block is removed.
        AIR = crate::id!(crate::ns::MINECRAFT, crate::word::AIR),
        /// The player entity's type.
        PLAYER = crate::id!(crate::ns::MINECRAFT, crate::word::PLAYER),
        /// The sound event that plays nothing: a block or item without a sound of its own.
        NO_SOUND = crate::id!(crate::ns::MINECRAFT, "intentionally_empty"),
        /// The biome a dimension is all of when it doesn't say.
        PLAINS = crate::id!(crate::ns::MINECRAFT, "plains"),
        THE_NETHER = crate::id!(crate::ns::MINECRAFT, "the_nether"),
        THE_END = crate::id!(crate::ns::MINECRAFT, "the_end"),
        /// The block tags of what each kind of tool mines: `<MINEABLE><kind>`.
        MINEABLE = crate::id!(crate::ns::MINECRAFT, "mineable/"),
        /// The texture the game draws where one is missing.
        MISSING_TEXTURE = crate::id!(crate::ns::MINECRAFT, "missingno"),
        /// What a book is given to players as.
        WRITTEN_BOOK = crate::id!(crate::ns::MINECRAFT, "written_book"),
        BOOK = crate::id!(crate::ns::MINECRAFT, crate::kind::BOOK),
        /// What the game's attributes were named under before 1.21.2: `generic.max_health`.
        OLD_ATTRIBUTE_PREFIX = crate::concatcp!(crate::word::GENERIC, crate::sep::DOT),
        /// The language every lang file falls back to, and a mod's texts are written in.
        LOCALE = "en_us",
    }


    /// The kinds of shape a schema describes itself with
    /// (`yog_data::Shape::describe`, read by the IDE's property grids).
    shape_kind {
        ANY = crate::word::ANY,
        BOOL = crate::word::BOOL,
        INT = crate::word::INT,
        FLOAT = "float",
        STRING = crate::word::STRING,
        ATOM = "atom",
        TEXT = crate::word::TEXT,
        IMAGE = crate::word::IMAGE,
        COLOR = crate::word::COLOR,
        BYTES = crate::word::BYTES,
        RES = "res",
        REF = "ref",
        TAG = crate::kind::TAG,
        REF_OR_TAG = "ref_or_tag",
        REF_OR_IMAGE = "ref_or_image",
        LIST = "list",
        MAP = "map",
        OPTIONAL = crate::word::OPTIONAL,
        STRUCT = "struct",
        ENUM = "enum",
        /// A struct or enum written once in `defs`, referred to by name.
        DEF = "def",
    }

    /// The elements of a `.yui` screen or component.
    yui {
        SCREEN = "Screen",
        COMPONENT = "Component",
        /// A component's declarations.
        PROPERTY = "Property",
        EVENT = "Event",
        STYLE = "Style",
        CANVAS = "Canvas",
        COLUMN = "Column",
        ROW = "Row",
        GRID = "Grid",
        /// A `<Grid>`'s row and column with limits, and the property elements
        /// that hold them, as WPF writes them.
        ROW_DEFINITION = "RowDefinition",
        COLUMN_DEFINITION = "ColumnDefinition",
        ROW_DEFINITIONS = crate::concatcp!(GRID, crate::sep::DOT, ROW_DEFINITION, crate::data_layout::PLURAL),
        COLUMN_DEFINITIONS = crate::concatcp!(GRID, crate::sep::DOT, COLUMN_DEFINITION, crate::data_layout::PLURAL),
        SCROLL_VIEW = "ScrollView",
        LABEL = "Label",
        BUTTON = "Button",
        TOGGLE_BUTTON = "ToggleButton",
        TEXT_BOX = "TextBox",
        IMAGE = "Image",
        VIDEO = "Video",
        SURFACE = "Surface",
        SPACER = "Spacer",
        STAT_BAR = "StatBar",
        LIST_VIEW = "ListView",
        ITEMS_CONTROL = "ItemsControl",
        CONTENT_PRESENTER = "ContentPresenter",
        /// Slots: an item shown, an inventory's, the container's.
        ITEM_SLOT = "ItemSlot",
        INV_SLOT = "InvSlot",
        SLOT = "Slot",
        PLAYER_INVENTORY = "PlayerInventory",
        /// Which container slot is which, for a block that makes recipes.
        MACHINE = "Machine",
        /// A kind of recipe's layout on an inventory screen.
        RECIPE_AREA = "RecipeArea",
        /// A recipe layout's places.
        RECIPE_SLOT = "RecipeSlot",
        RECIPE_RESULT = "RecipeResult",
        RECIPE_COUNT = "RecipeCount",
        /// A block in 3D, at a slot's angle (a recipe layout's station).
        BLOCK_SLOT = "BlockSlot",
        /// A book page type's elements.
        BOOK_HEADER = "BookHeader",
        BOOK_TEXT = "BookText",
        BOOK_ITEM = "BookItem",
        /// A picture the mod's code draws.
        DRAWN = "Drawn",
    }

    /// A request to the CLI's service (`yog serve`): the method and its parameters.
    serve_request {
        METHOD = "method",
        PARAMS = "params",
    }

    /// The parameters of the CLI's service methods (`yog serve`, what the IDE calls).
    serve_param {
        PATH = "path",
        NAME = crate::word::NAME,
        TEXT = crate::word::TEXT,
        SIZE = crate::word::SIZE,
        TEXTURE = crate::kind::TEXTURE,
        SOUND = crate::kind::SOUND,
        PNG = crate::word::PNG,
        ANIMATION = crate::word::ANIMATION,
        KIND = crate::word::KIND,
        REGISTRY = "registry",
        ID = crate::word::ID,
        ALIAS = "alias",
        MODEL = crate::word::MODEL,
        SOURCE = crate::word::SOURCE,
        FORCE = "force",
        WIDTH = crate::word::WIDTH,
        HEIGHT = crate::word::HEIGHT,
        TAG = crate::kind::TAG,
        ARGS = "args",
        EVENT = crate::word::EVENT,
        FILE = crate::word::FILE,
        SET = crate::bus_word::SET,
        MOD_ID = crate::word::MOD,
        ENTRIES = crate::word::ENTRIES,
        NEXT = "next",
        LEVEL = crate::kind::LEVEL,
        VALUE = crate::word::VALUE,
        VALUES = crate::word::VALUES,
        FORMAT = "format",
        FRAME = crate::word::FRAME,
        KEY = crate::word::KEY,
    }

    /// The methods of `yog serve` (the CLI as a service), by the name a tool
    /// (Yog-IDLE) calls them; their parameters are `serve_param`.
    serve_method {
        ADVANCEMENT_RENDER = "advancement_render",
        BODY_TEXTURE = "body_texture",
        BOOK_DESIGNER_CLOSE = "book_designer_close",
        BOOK_DESIGNER_RENDER = "book_designer_render",
        BOOK_PAGE_PREVIEW = "book_page_preview",
        CONTENT_ARMOR_MATERIALS = "content_armor_materials",
        CONTENT_BLOCK_KINDS = "content_block_kinds",
        CONTENT_DEFS = "content_defs",
        CONTENT_IMPORT = "content_import",
        CONTENT_ITEM_POTION_COLOR = "content_item_potion_color",
        CONTENT_MOD_ID = "content_mod_id",
        CONTENT_PARENT_MODEL = "content_parent_model",
        CONTENT_REFS = "content_refs",
        CONTENT_SAVE_TEXTURE = "content_save_texture",
        CONTENT_SAVE_TEXTURE_ANIMATION = "content_save_texture_animation",
        CONTENT_SOUND_GROUPS = "content_sound_groups",
        CONTENT_STRINGS = "content_strings",
        CONTENT_TAGS = "content_tags",
        CONTENT_TEXTURE = "content_texture",
        CONTENT_TEXTURES = "content_textures",
        CONTENT_TOOL_MATERIALS = "content_tool_materials",
        DATA_FORMAT = "data.format",
        DATA_PARSE = "data.parse",
        DATA_PATCH = "data.patch",
        FONT_COPY = "font_copy",
        FORMAT_COMPLETE = "format.complete",
        FORMAT_SHAPE = "format.shape",
        ITEM_TAG_ITEMS = "item_tag_items",
        MEDIA_IMPORT = "media.import",
        MEDIA_PICTURES = "media.pictures",
        MOD_ID = "mod.id",
        RECIPE_KIND_CREATE = "recipe_kind_create",
        RECIPE_KINDS = "recipe_kinds",
        SOUND_DATA = "sound_data",
        SOUND_LIST = "sound_list",
        /// The mod's videos (`@Videos.intro`), what a `.yvideo` can show.
        VIDEO_LIST = "video_list",
        /// A `.yvideo` as edited, rendered as the build renders it.
        VIDEO_PREVIEW = "video_preview",
        /// A render's frame, decoded as the game decodes it (PNG).
        VIDEO_FRAME = "video_frame",
        SOURCE_CHECK = "source.check",
        SOURCE_READ = "source.read",
        SOURCE_WRITE = "source.write",
        TEXTURE_ANIMATION_FORMAT = "texture_animation_format",
        TEXTURE_ANIMATION_RESOLVE = "texture_animation_resolve",
        TEXTURE_FILE_ID = "texture_file_id",
        TYPED_CLASSIFY = "typed.classify",
        VERSION = "version",
        WORLDGEN_PREVIEW = "worldgen_preview",
        YRES_PARSE = "yres.parse",
        YRES_WRITE = "yres.write",
        YUI_CODEGEN = "yui_codegen",
        YUI_DESIGNER_COMPONENTS = "yui_designer_components",
        YUI_DESIGNER_EVENT = "yui_designer_event",
        YUI_DESIGNER_RENDER = "yui_designer_render",
        YUI_DESIGNER_RESET = "yui_designer_reset",
        YUI_EVENT_HANDLER = "yui_event_handler",
        YUI_IMAGE_PREVIEW = "yui_image_preview",
        YUI_IMAGE_SOURCES = "yui_image_sources",
    }

    /// The fields of the game's tag files (`data/<ns>/tags/<registry>/<path>.json`).
    tag_file {
        VALUES = crate::word::VALUES,
        /// What a loader reads alongside `values` (Forge's).
        OPTIONAL = crate::word::OPTIONAL,
        REPLACE = "replace",
        /// An entry that may be missing: `{ "id": …, "required": false }`.
        ID = crate::word::ID,
        REQUIRED = "required",
    }

    /// The game's atlas definitions (`atlases/<atlas>.json`): their sources —
    /// a whole texture (`single`), or sprites cut from one picture
    /// (`unstitch`, its regions).
    atlas_source {
        SOURCES = "sources",
        TYPE = crate::word::TYPE,
        SINGLE = "single",
        UNSTITCH = "unstitch",
        RESOURCE = "resource",
        SPRITE = crate::word::SPRITE,
        DIVISOR_X = "divisor_x",
        DIVISOR_Y = "divisor_y",
        REGIONS = "regions",
        X = crate::word::X,
        Y = crate::word::Y,
        WIDTH = crate::word::WIDTH,
        HEIGHT = crate::word::HEIGHT,
        /// The game's atlases of blocks and of items.
        BLOCKS = crate::word::BLOCKS,
        ITEMS = crate::word::ITEMS,
    }

    /// The texture variables of the game's block models (`cube_all`'s `all`, a
    /// column's `end`, …) — beside the faces' own names (`up`, `north`, …).
    texture_slot {
        ALL = "all",
        SIDE = "side",
        TOP = crate::word::TOP,
        BOTTOM = crate::word::BOTTOM,
        END = crate::word::END,
        FRONT = "front",
        /// A one-texture model's.
        TEXTURE = crate::word::TEXTURE,
    }

    /// The game's block state properties Yog's blocks get, and their values.
    block_state {
        /// The way a block faces (a `direction`): the four sides, or up and down too.
        FACING = "facing",
        /// What an attached block hangs on.
        FACE = "face",
        FLOOR = "floor",
        WALL = crate::word::WALL,
        CEILING = "ceiling",
        /// Whether it stands in water.
        WATERLOGGED = "waterlogged",
        /// A yes/no property's values.
        FALSE = crate::word::FALSE,
        TRUE = crate::word::TRUE,
    }

    /// The fields and kinds of the game's loot tables (`data/<ns>/loot_table/…`).
    loot {
        TYPE = crate::word::TYPE,
        POOLS = "pools",
        ROLLS = "rolls",
        ENTRIES = crate::word::ENTRIES,
        NAME = crate::word::NAME,
        CONDITIONS = "conditions",
        CONDITION = "condition",
        FUNCTIONS = "functions",
        FUNCTION = crate::word::FUNCTION,
        COUNT = crate::word::COUNT,
        MIN = "min",
        MAX = "max",
        BLOCK = crate::word::BLOCK,
        PROPERTIES = "properties",
        /// A block's table, and one nothing uses the way of.
        BLOCK_TABLE = crate::id!(crate::ns::MINECRAFT, crate::word::BLOCK),
        GENERIC_TABLE = crate::id!(crate::ns::MINECRAFT, crate::word::GENERIC),
        /// An entry of an item.
        ITEM_ENTRY = crate::id!(crate::ns::MINECRAFT, crate::word::ITEM),
        /// A number between `min` and `max`.
        UNIFORM = crate::id!(crate::ns::MINECRAFT, crate::game_font::UNIFORM),
        SET_COUNT = crate::id!(crate::ns::MINECRAFT, "set_count"),
        EXPLOSION_DECAY = crate::id!(crate::ns::MINECRAFT, "explosion_decay"),
        SURVIVES_EXPLOSION = crate::id!(crate::ns::MINECRAFT, "survives_explosion"),
        BLOCK_STATE_PROPERTY = crate::id!(crate::ns::MINECRAFT, "block_state_property"),
        /// Where blocks' tables are: `loot_table/blocks/<path>`.
        BLOCKS = crate::concatcp!(crate::word::BLOCKS, crate::sep::PATH),
    }

    /// The fields of a dimension as the build hands it to the runtime and
    /// the runtime to the host.
    dimension_field {
        ID = crate::word::ID,
        DIMENSION_TYPE = crate::concatcp!(crate::kind::DIMENSION, crate::sep::WORD, crate::word::TYPE),
        BIOME = crate::kind::BIOME,
        STRUCTURES = "structures",
        FEATURES = "features",
        MOB_SPAWNING = "mob_spawning",
        /// The generator its chunks come from: its own terrain's, or one in code.
        GENERATOR_TYPE = "generator_type",
        /// Its terrain, which the runtime keeps (the host never sees it).
        TERRAIN = "terrain",
    }

    /// Words a `.yui` attribute's value is.
    yui_value {
        TRUE = crate::word::TRUE,
        FALSE = crate::word::FALSE,
        /// A size the content decides.
        AUTO = crate::word::AUTO,
        START = crate::word::START,
        CENTER = crate::word::CENTER,
        END = crate::word::END,
        LEFT = crate::word::LEFT,
        RIGHT = crate::word::RIGHT,
        TOP = crate::word::TOP,
        BOTTOM = crate::word::BOTTOM,
        /// Takes the room there is.
        FILL = crate::word::FILL,
        STRETCH = crate::word::STRETCH,
        OUTLINE = crate::word::OUTLINE,
        NONE = crate::word::NONE,
    }

    /// The markup of a `.yui` attribute's value: a binding (`{x:Bind name}`)
    /// opens and closes so.
    yui_markup {
        BIND_OPEN = "{x:Bind ",
        BIND_CLOSE = "}",
        /// A binding's start, either way it's written (`{x:Bind …}`, `{Binding …}`).
        BIND = "{x:Bind",
        BINDING = "{Binding",
        /// A design-time attribute: `d:text="Sample"` shows in the designer only.
        DESIGN = "d:",
        /// What a screen's resources element ends with: `<Screen.Resources>`.
        RESOURCES = crate::concatcp!(crate::sep::DOT, "Resources"),
    }


    /// The game's kinds of tool (`item.tool.kind`); a mod's kind is the
    /// block tag of what it mines.
    tool_kind {
        PICKAXE = "pickaxe",
        AXE = "axe",
        SHOVEL = "shovel",
        HOE = "hoe",
        /// A weapon: it mines nothing of its own.
        SWORD = "sword",
    }

    /// The fields of the game's model JSON (block and item models, block states' variants).
    model {
        PARENT = "parent",
        TEXTURES = "textures",
        ELEMENTS = "elements",
        DISPLAY = crate::word::DISPLAY,
        FACES = "faces",
        FROM = "from",
        TO = "to",
        UV = crate::word::UV,
        ROTATION = "rotation",
        TEXTURE = crate::word::TEXTURE,
        /// A sprite item's (`item/generated`) first layer.
        LAYER0 = "layer0",
        /// The texture a block's particles take, and what a face falls back to.
        PARTICLE = crate::word::PARTICLE,
        /// A block state variant's model.
        MODEL = crate::word::MODEL,
        /// An element's rotation.
        ORIGIN = "origin",
        AXIS = "axis",
        ANGLE = "angle",
        RESCALE = "rescale",
        /// Block states: variants by property values, or parts applied when they match.
        VARIANTS = "variants",
        MULTIPART = "multipart",
        WHEN = crate::word::WHEN,
        APPLY = "apply",
        OR = "OR",
        AND = "AND",
        /// How a block's model is drawn (Forge/NeoForge read it; Fabric ignores it).
        RENDER_TYPE = "render_type",
        /// An item sprite's layers: `layer0`, `layer1`, …
        LAYER = "layer",
        SHADE = "shade",
        CULLFACE = "cullface",
        TINTINDEX = "tintindex",
        AMBIENT_OCCLUSION = "ambientocclusion",
        GUI_LIGHT = "gui_light",
        TRANSLATION = "translation",
        SCALE = "scale",
        /// The parent a block model of its own elements takes: how it's held and shown in a slot.
        BLOCK_PARENT = crate::id!(crate::ns::MINECRAFT, crate::concatcp!(crate::res::BLOCK_DIR, crate::kind::BLOCK)),
        X = crate::word::X,
        Y = crate::word::Y,
        UVLOCK = "uvlock",
        WEIGHT = "weight",
    }

    /// The fields of Yog's content files (`.yblock`, `.yitem`, `.ydef`) the tools read directly.
    content_field {
        ID = crate::word::ID,
        KIND = crate::word::KIND,
        MODEL = crate::word::MODEL,
        ITEM_MODEL = crate::concatcp!(crate::kind::ITEM, crate::sep::WORD, crate::word::MODEL),
        MATERIAL = crate::kind::MATERIAL,
        ITEM = crate::kind::ITEM,
        POTION = crate::kind::POTION,
        EFFECTS = "effects",
        EFFECT = crate::kind::EFFECT,
        OF = "of",
        COLOR = crate::word::COLOR,
    }

    /// The attributes of `.yui` elements.
    yui_attr {
        ID = crate::word::ID,
        X = crate::word::X,
        Y = crate::word::Y,
        W = "w",
        H = "h",
        MIN_W = "min_w",
        MIN_H = "min_h",
        FLEX = "flex",
        DOCK = "dock",
        GAP = "gap",
        PADDING = "padding",
        BG = "bg",
        COLOR = crate::word::COLOR,
        ALIGN = "align",
        HALIGN = "halign",
        VALIGN = "valign",
        MARGIN = "margin",
        FONT_SCALE = "font_scale",
        FONT = crate::kind::FONT,
        SHADOW = "shadow",
        NO_WRAP = "no_wrap",
        ELLIPSIS = "ellipsis",
        FOCUS_STYLE = "focus_style",
        FOCUS_COLOR = "focus_color",
        ENABLED = "enabled",
        ON_CLICK = "on_click",
        TOOLTIP = "tooltip",
        WIDTH = crate::word::WIDTH,
        HEIGHT = crate::word::HEIGHT,
        ANCHOR = "anchor",
        MODAL = "modal",
        PAUSE = "pause",
        LIVE = "live",
        THEME = "theme",
        TEXT = crate::word::TEXT,
        PLACEHOLDER = "placeholder",
        MAX_LENGTH = "max_length",
        ON_CHANGE = "on_change",
        ON_SUBMIT = "on_submit",
        ITEM = crate::kind::ITEM,
        COUNT = crate::word::COUNT,
        INDEX = "index",
        ACCEPTS = "accepts",
        SOURCE = crate::word::SOURCE,
        STRETCH = crate::word::STRETCH,
        UV = crate::word::UV,
        TINT = "tint",
        KIND = crate::word::KIND,
        RESULT = crate::word::RESULT,
        PLACES = "places",
        RECIPE_LAYOUT = "recipe_layout",
        SLOT = "slot",
        BLOCK = crate::kind::BLOCK,
        /// A slot frame behind it, for layouts without their own picture.
        FRAME = crate::word::FRAME,
        SIZE = crate::word::SIZE,
        /// A `<Component>` that is a kind of book page, its id.
        BOOK_PAGE = "book_page",
        /// A `<Property>`'s.
        NAME = crate::word::NAME,
        TYPE = crate::word::TYPE,
        DEFAULT = crate::word::DEFAULT,
        /// A book page's image and drawn picture.
        SRC = crate::word::SRC,
        DRAWER = "drawer",
        DATA = crate::word::DATA,
        STYLE = "style",
        BASED_ON = "based_on",
        ORIENTATION = "orientation",
        /// A `<Grid>`'s tracks (`rows="Auto,*,26"`), the room between them,
        /// and a `RowDefinition`'s / `ColumnDefinition`'s limits.
        ROWS = "rows",
        COLUMNS = "columns",
        ROW_SPACING = "row_spacing",
        COLUMN_SPACING = "column_spacing",
        MIN_HEIGHT = "min_height",
        MAX_HEIGHT = "max_height",
        MIN_WIDTH = "min_width",
        MAX_WIDTH = "max_width",
        VALUE = crate::word::VALUE,
        SELECTED = "selected",
        CHECKED = "checked",
        ITEM_HEIGHT = "item_height",
        SCROLL_KEY = "scroll_key",
        VISIBLE = "visible",
        /// A resource's or style's key, an element's name (its id), what a style is for.
        X_KEY = "x:Key",
        X_NAME = "x:Name",
        /// A view-model screen's type, how it's made, a template's item type.
        X_VIEW_MODEL = "x:ViewModel",
        X_CREATE = "x:Create",
        X_DATA_TYPE = "x:DataType",
        TARGET = crate::word::TARGET,
        GRID_ROW = crate::concatcp!(crate::yui::GRID, crate::sep::DOT, crate::yui::ROW),
        GRID_COLUMN = crate::concatcp!(crate::yui::GRID, crate::sep::DOT, crate::yui::COLUMN),
        GRID_ROWSPAN = crate::concatcp!(crate::yui::GRID, crate::sep::DOT, "RowSpan"),
        GRID_COLUMNSPAN = crate::concatcp!(crate::yui::GRID, crate::sep::DOT, "ColumnSpan"),
    }
}
