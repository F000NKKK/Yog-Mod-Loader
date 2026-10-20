//! Resources, files and content: where things live and what they're called.

names! {

    /// A `.yres` entry's type (`{ "type": "sound", ... }`).
    res_kind {
        STRING = "string",
        IMAGE = crate::typed::IMAGE,
        FILE = "file",
        SOUND = "sound",
        COLOR = "color",
        NUMBER = "number",
        BOOL = "bool",
    }

    /// What an image source starts with (`item:apple`).
    image_scheme {
        ITEM = crate::kind::ITEM,
        BLOCK = crate::kind::BLOCK,
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

    /// The pack the build writes for the hosts.
    pack {
        /// The game's data: `data/<ns>/…`.
        DATA = "data",
        /// The game's assets: `assets/<ns>/…`.
        ASSETS = "assets",
        /// Tags, under a namespace's data: `data/<ns>/tags/<registry>/<path>.json`.
        TAGS = "tags",
        /// Overlays for one loader and version: `when/<loader>/<version>/<file>`.
        OVERLAYS = "when",
        /// An overlay's loader when it's for every loader.
        ANY_LOADER = "any",
        /// An overlay's version when it's for every version.
        ANY_VERSION = "any",
    }

    /// A mod project's folders.
    project {
        /// Its resources: `.yres` sets, `.ytexture`s, ….
        RESOURCES = "resources",
        /// Its content: blocks, items, recipes, books, definitions.
        CONTENT = "content",
        /// Where a copy of a game font goes, under its content.
        FONT_COPIES = "font",
        /// Its code and screens.
        SRC = "src",
        /// Where `yog build` writes: the generated crate, generated code, listings.
        BUILD = ".yog-build",
    }

    /// A game's folders Yog uses.
    game_dir {
        /// The `.yog` mods the game loads: `<game>/yog-mods`.
        MODS = "yog-mods",
    }

    /// Yog's folders in a user's cache (`~/.cache/<CACHE>/…`).
    user_cache {
        CACHE = "yog",
        /// Compiled shaders, by their sources' hash.
        SHADERS = "shaders",
    }

    /// Yog's programs.
    program {
        /// The command-line tool (`yog build`, `yog schema`, …).
        CLI = "yog",
        /// The CLI's service command: tools read and write every format through it.
        SERVE = "serve",
    }

    /// Where the game keeps resources, and what their files end with.
    res {
        /// The textures' folder: `<ns>:textures/<path>.png`.
        TEXTURES = crate::concatcp!("textures", crate::sep::PATH),
        /// The models' folder: `<ns>:models/<path>.json`.
        MODELS = crate::concatcp!("models", crate::sep::PATH),
        /// The block states' folder: `<ns>:blockstates/<path>.json`.
        BLOCKSTATES = crate::concatcp!("blockstates", crate::sep::PATH),
        /// Items' textures and models: `item/apple`.
        ITEM_DIR = crate::concatcp!(crate::kind::ITEM, crate::sep::PATH),
        /// Item definitions (1.21.4+): `<ns>:items/<path>.json`.
        ITEM_DEFINITIONS = crate::concatcp!("items", crate::sep::PATH),
        /// Equipment's worn looks (1.21.4+): `<ns>:equipment/<path>.json`.
        EQUIPMENT = crate::concatcp!("equipment", crate::sep::PATH),
        /// Worn equipment's textures (1.21.4+): `entity/equipment/<layer>/<path>`.
        EQUIPMENT_TEXTURES = crate::concatcp!("entity", crate::sep::PATH, "equipment", crate::sep::PATH),
        /// Armor's worn textures before 1.21.4: `models/armor/<path>_layer_<n>`.
        ARMOR_TEXTURES = crate::concatcp!("models", crate::sep::PATH, "armor", crate::sep::PATH),
        /// Between an armor texture's name and its layer's number.
        ARMOR_LAYER = "_layer_",
        /// What a spawn egg's item path ends with: `<mob>_spawn_egg`.
        SPAWN_EGG_SUFFIX = "_spawn_egg",
        /// Blocks' textures and models: `block/stone`.
        BLOCK_DIR = crate::concatcp!(crate::kind::BLOCK, crate::sep::PATH),
        /// The GUI atlas's sprites under `textures/` (1.20.2+).
        GUI_SPRITES = crate::concatcp!("gui", crate::sep::PATH, "sprites", crate::sep::PATH),
        /// The atlases' definitions: `<ns>:atlases/<atlas>.json` (every pack's sources combine).
        ATLASES = crate::concatcp!("atlases", crate::sep::PATH),
        /// The language files' folder: `<ns>:lang/<locale>.json`.
        LANG = crate::concatcp!("lang", crate::sep::PATH),
        /// Models the game draws in code (`builtin/entity`), not from elements.
        BUILTIN = crate::concatcp!("builtin", crate::sep::PATH),
        PNG = ".png",
        SVG = ".svg",
        JSON = ".json",
        MCMETA = ".mcmeta",
    }

    /// Yog's own ids.
    yog {
        /// An entity's data the hosts keep for the mods.
        STATE = crate::id!(crate::ns::YOG, "state"),
        /// A hanging entity's attachment (its face, size, item).
        HANGING = crate::id!(crate::ns::YOG, "hanging"),
        /// The channel entity data is synced on.
        ENTITY_DATA = crate::id!(crate::ns::YOG, "entity_data"),
        /// The entity data key an entity's own hitbox travels under: width,
        /// height, eye height as three little-endian f32.
        DIMENSIONS = crate::id!(crate::ns::YOG, "dimensions"),
        /// An inventory slot widget's id: `yog:slot:<index>`.
        SLOT_PREFIX = crate::id!(crate::ns::YOG, "slot:"),
        /// The player's inventory widget.
        PLAYER_INVENTORY = crate::id!(crate::ns::YOG, "player_inventory"),
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
        SPAWN_EGG_MODEL = crate::id!(crate::ns::YOG, "item/spawn_egg"),
        /// The book's pictures.
        BOOK_SHEET = crate::id!(crate::ns::YOG, "gui/book/book"),
        CRAFTING_SHEET = crate::id!(crate::ns::YOG, "gui/book/crafting"),
        /// A screen being designed in the IDE.
        DESIGNER_SCREEN = crate::id!(crate::ns::YOG, "designer"),
        /// A book's default texture (its sheet is [`BOOK_SHEET`]).
        BOOK_TEXTURE = crate::id!(crate::ns::YOG, "gui/book"),
        /// An image source that draws nothing (a bare surface).
        NO_IMAGE = crate::id!(crate::ns::YOG, "none"),
        /// The textures the runtime renders SVG images into: `<prefix><key>_<w>x<h>`.
        DYNAMIC_SVG_PREFIX = crate::id!(crate::ns::YOG, "dynamic/svg_"),
        /// The textures the runtime draws text in Yog's fonts into: `<prefix><n>`.
        DYNAMIC_TEXT_PREFIX = crate::id!(crate::ns::YOG, "dynamic/text_"),
    }

    /// The game's fonts, by name: Yog aliases each as
    /// `yog:font/<name>` ([`crate::yog::FONT_ALIAS`]).
    game_font {
        DEFAULT = "default",
        /// Unifont.
        UNIFORM = "uniform",
        /// The enchanting table's.
        ALT = "alt",
        ILLAGERALT = "illageralt",
    }

    /// The game's ids Yog's code names.
    game {
        /// The game's own id of its default font (hosts only; everything
        /// else says [`crate::yog::DEFAULT_FONT`]).
        DEFAULT_FONT = crate::id!(crate::ns::MINECRAFT, crate::game_font::DEFAULT),
        /// The UI theme drawn like the game's own screens.
        VANILLA_THEME = crate::id!(crate::ns::MINECRAFT, "vanilla"),
        OVERWORLD = crate::id!(crate::ns::MINECRAFT, "overworld"),
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
        BOOK = crate::id!(crate::ns::MINECRAFT, "book"),
        /// The language every lang file falls back to, and a mod's texts are written in.
        LOCALE = "en_us",
    }

    /// What `[external.<namespace>]` in `yog.toml` lists another mod's content under.
    external {
        ITEMS = "items",
        BLOCKS = "blocks",
        ENTITIES = "entities",
        RECIPES = "recipes",
    }

    /// The kinds of shape a schema describes itself with
    /// (`yog_data::Shape::describe`, read by the IDE's property grids).
    shape_kind {
        ANY = "any",
        BOOL = "bool",
        INT = "int",
        FLOAT = "float",
        STRING = "string",
        ATOM = "atom",
        TEXT = "text",
        IMAGE = "image",
        COLOR = "color",
        BYTES = "bytes",
        RES = "res",
        REF = "ref",
        TAG = "tag",
        REF_OR_TAG = "ref_or_tag",
        REF_OR_IMAGE = "ref_or_image",
        LIST = "list",
        MAP = "map",
        OPTIONAL = "optional",
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
        SCROLL_VIEW = "ScrollView",
        LABEL = "Label",
        BUTTON = "Button",
        TOGGLE_BUTTON = "ToggleButton",
        TEXT_BOX = "TextBox",
        IMAGE = "Image",
        SURFACE = "Surface",
        SPACER = "Spacer",
        PILL = "Pill",
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

    /// The fields and types of the game's item definitions (`items/<path>.json`, 1.21.4+).
    item_def {
        MODEL = "model",
        TYPE = "type",
        TINTS = "tints",
        VALUE = "value",
        /// A plain model.
        PLAIN = "minecraft:model",
        /// A tint of one colour.
        CONSTANT = "minecraft:constant",
    }

    /// The fields and layers of the game's equipment assets (`equipment/<path>.json`, 1.21.4+).
    equipment {
        LAYERS = "layers",
        TEXTURE = "texture",
        /// Worn on the body: helmet, chestplate, boots.
        HUMANOID = "humanoid",
        /// Worn on the legs: leggings.
        HUMANOID_LEGGINGS = "humanoid_leggings",
    }

    /// The game's block state properties Yog's blocks get, and their values.
    block_state {
        /// The way a block faces: the four sides, or up and down too.
        FACING = "facing",
        NORTH = "north",
        EAST = "east",
        SOUTH = "south",
        WEST = "west",
        UP = "up",
        DOWN = "down",
        /// What an attached block hangs on.
        FACE = "face",
        FLOOR = "floor",
        WALL = "wall",
        CEILING = "ceiling",
        /// Whether it stands in water.
        WATERLOGGED = "waterlogged",
        /// A yes/no property's values.
        FALSE = "false",
        TRUE = "true",
    }

    /// The fields and kinds of the game's loot tables (`data/<ns>/loot_table/…`).
    loot {
        TYPE = "type",
        POOLS = "pools",
        ROLLS = "rolls",
        ENTRIES = "entries",
        NAME = "name",
        CONDITIONS = "conditions",
        CONDITION = "condition",
        FUNCTIONS = "functions",
        FUNCTION = "function",
        COUNT = "count",
        MIN = "min",
        MAX = "max",
        BLOCK = "block",
        PROPERTIES = "properties",
        /// A block's table, and one nothing uses the way of.
        BLOCK_TABLE = crate::id!(crate::ns::MINECRAFT, "block"),
        GENERIC_TABLE = crate::id!(crate::ns::MINECRAFT, "generic"),
        /// An entry of an item.
        ITEM_ENTRY = crate::id!(crate::ns::MINECRAFT, "item"),
        /// A number between `min` and `max`.
        UNIFORM = crate::id!(crate::ns::MINECRAFT, "uniform"),
        SET_COUNT = crate::id!(crate::ns::MINECRAFT, "set_count"),
        EXPLOSION_DECAY = crate::id!(crate::ns::MINECRAFT, "explosion_decay"),
        SURVIVES_EXPLOSION = crate::id!(crate::ns::MINECRAFT, "survives_explosion"),
        BLOCK_STATE_PROPERTY = crate::id!(crate::ns::MINECRAFT, "block_state_property"),
        /// Where blocks' tables are: `loot_table/blocks/<path>`.
        BLOCKS = crate::concatcp!("blocks", crate::sep::PATH),
    }

    /// The fields of a dimension as the build hands it to the runtime and
    /// the runtime to the host.
    dimension_field {
        ID = "id",
        DIMENSION_TYPE = "dimension_type",
        BIOME = "biome",
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
        TRUE = "true",
        FALSE = "false",
        /// A size the content decides.
        AUTO = "auto",
        START = "start",
        CENTER = "center",
        END = "end",
    }

    /// The markup of a `.yui` attribute's value: a binding (`{x:Bind name}`)
    /// opens and closes so.
    yui_markup {
        BIND_OPEN = "{x:Bind ",
        BIND_CLOSE = "}",
    }

    /// The sections of the game's `.mcmeta` beside a texture.
    mcmeta {
        ANIMATION = "animation",
        TEXTURE = "texture",
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
        DISPLAY = "display",
        FACES = "faces",
        FROM = "from",
        TO = "to",
        UV = "uv",
        ROTATION = "rotation",
        TEXTURE = "texture",
        /// A sprite item's (`item/generated`) first layer.
        LAYER0 = "layer0",
        /// The texture a block's particles take, and what a face falls back to.
        PARTICLE = "particle",
        /// A block state variant's model.
        MODEL = "model",
        /// An element's rotation.
        ORIGIN = "origin",
        AXIS = "axis",
        ANGLE = "angle",
        RESCALE = "rescale",
        /// Block states: variants by property values, or parts applied when they match.
        VARIANTS = "variants",
        MULTIPART = "multipart",
        WHEN = "when",
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
        BLOCK_PARENT = crate::id!(crate::ns::MINECRAFT, "block/block"),
        X = "x",
        Y = "y",
        UVLOCK = "uvlock",
        WEIGHT = "weight",
    }

    /// The fields of Yog's content files (`.yblock`, `.yitem`, `.ydef`) the tools read directly.
    content_field {
        ID = "id",
        KIND = "kind",
        MODEL = "model",
        ITEM_MODEL = "item_model",
        MATERIAL = "material",
        ITEM = "item",
        POTION = "potion",
        EFFECTS = "effects",
        EFFECT = "effect",
        OF = "of",
        COLOR = "color",
    }

    /// The attributes of `.yui` elements.
    yui_attr {
        ID = "id",
        X = "x",
        Y = "y",
        W = "w",
        H = "h",
        MIN_W = "min_w",
        MIN_H = "min_h",
        FLEX = "flex",
        DOCK = "dock",
        GAP = "gap",
        PADDING = "padding",
        BG = "bg",
        COLOR = "color",
        ALIGN = "align",
        HALIGN = "halign",
        VALIGN = "valign",
        MARGIN = "margin",
        FONT_SCALE = "font_scale",
        FONT = "font",
        SHADOW = "shadow",
        NO_WRAP = "no_wrap",
        ELLIPSIS = "ellipsis",
        FOCUS_STYLE = "focus_style",
        FOCUS_COLOR = "focus_color",
        ENABLED = "enabled",
        ON_CLICK = "on_click",
        TOOLTIP = "tooltip",
        WIDTH = "width",
        HEIGHT = "height",
        ANCHOR = "anchor",
        MODAL = "modal",
        PAUSE = "pause",
        LIVE = "live",
        THEME = "theme",
        TEXT = "text",
        PLACEHOLDER = "placeholder",
        MAX_LENGTH = "max_length",
        ON_CHANGE = "on_change",
        ON_SUBMIT = "on_submit",
        ITEM = "item",
        COUNT = "count",
        INDEX = "index",
        ACCEPTS = "accepts",
        SOURCE = "source",
        STRETCH = "stretch",
        UV = "uv",
        TINT = "tint",
        KIND = "kind",
        RESULT = "result",
        PLACES = "places",
        RECIPE_LAYOUT = "recipe_layout",
        SLOT = "slot",
        BLOCK = "block",
        /// A slot frame behind it, for layouts without their own picture.
        FRAME = "frame",
        SIZE = "size",
        /// A `<Component>` that is a kind of book page, its id.
        BOOK_PAGE = "book_page",
        /// A `<Property>`'s.
        NAME = "name",
        TYPE = "type",
        DEFAULT = "default",
        /// A book page's image and drawn picture.
        SRC = "src",
        DRAWER = "drawer",
        DATA = "data",
        STYLE = "style",
        BASED_ON = "based_on",
        ORIENTATION = "orientation",
        ROWS = "rows",
        COLUMNS = "columns",
        ROW_SPACING = "row_spacing",
        COLUMN_SPACING = "column_spacing",
        VALUE = "value",
        SELECTED = "selected",
        CHECKED = "checked",
        ITEM_HEIGHT = "item_height",
        SCROLL_KEY = "scroll_key",
        VISIBLE = "visible",
        GRID_ROW = "Grid.Row",
        GRID_COLUMN = "Grid.Column",
        GRID_ROWSPAN = "Grid.RowSpan",
        GRID_COLUMNSPAN = "Grid.ColumnSpan",
    }
}
