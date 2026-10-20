//! Resources, files and content: where things live and what they're called.

names! {

    /// A `.yres` entry's type (`{ "type": "sound", ... }`).
    res_kind {
        STRING = "string",
        IMAGE = typed::IMAGE,
        FILE = "file",
        SOUND = "sound",
        COLOR = "color",
        NUMBER = "number",
        BOOL = "bool",
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
        TEXTURES = concatcp!("textures", sep::PATH),
        /// The models' folder: `<ns>:models/<path>.json`.
        MODELS = concatcp!("models", sep::PATH),
        /// The block states' folder: `<ns>:blockstates/<path>.json`.
        BLOCKSTATES = concatcp!("blockstates", sep::PATH),
        /// Items' textures and models: `item/apple`.
        ITEM_DIR = concatcp!(kind::ITEM, sep::PATH),
        /// Item definitions (1.21.4+): `<ns>:items/<path>.json`.
        ITEM_DEFINITIONS = concatcp!("items", sep::PATH),
        /// Equipment's worn looks (1.21.4+): `<ns>:equipment/<path>.json`.
        EQUIPMENT = concatcp!("equipment", sep::PATH),
        /// Worn equipment's textures (1.21.4+): `entity/equipment/<layer>/<path>`.
        EQUIPMENT_TEXTURES = concatcp!("entity", sep::PATH, "equipment", sep::PATH),
        /// Armor's worn textures before 1.21.4: `models/armor/<path>_layer_<n>`.
        ARMOR_TEXTURES = concatcp!("models", sep::PATH, "armor", sep::PATH),
        /// Between an armor texture's name and its layer's number.
        ARMOR_LAYER = "_layer_",
        /// What a spawn egg's item path ends with: `<mob>_spawn_egg`.
        SPAWN_EGG_SUFFIX = "_spawn_egg",
        /// Blocks' textures and models: `block/stone`.
        BLOCK_DIR = concatcp!(kind::BLOCK, sep::PATH),
        /// The GUI atlas's sprites under `textures/` (1.20.2+).
        GUI_SPRITES = concatcp!("gui", sep::PATH, "sprites", sep::PATH),
        /// The atlases' definitions: `<ns>:atlases/<atlas>.json` (every pack's sources combine).
        ATLASES = concatcp!("atlases", sep::PATH),
        /// The language files' folder: `<ns>:lang/<locale>.json`.
        LANG = concatcp!("lang", sep::PATH),
        /// Models the game draws in code (`builtin/entity`), not from elements.
        BUILTIN = concatcp!("builtin", sep::PATH),
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
        /// Yog's alias of a game font, `yog:font/<name>` — the only way
        /// anything names one of the game's fonts.
        FONT_ALIAS = id!(ns::YOG, "font/"),
        /// The font text is drawn in by default (the game's).
        DEFAULT_FONT = concatcp!(FONT_ALIAS, game_font::DEFAULT),
        /// The model every mod's spawn egg inherits: Yog's egg in two layers, base and spots.
        SPAWN_EGG_MODEL = id!(ns::YOG, "item/spawn_egg"),
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
        /// The textures the runtime draws text in Yog's fonts into: `<prefix><n>`.
        DYNAMIC_TEXT_PREFIX = id!(ns::YOG, "dynamic/text_"),
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
        DEFAULT_FONT = id!(ns::MINECRAFT, game_font::DEFAULT),
        /// The UI theme drawn like the game's own screens.
        VANILLA_THEME = id!(ns::MINECRAFT, "vanilla"),
        OVERWORLD = id!(ns::MINECRAFT, "overworld"),
        THE_NETHER = id!(ns::MINECRAFT, "the_nether"),
        THE_END = id!(ns::MINECRAFT, "the_end"),
        /// The texture the game draws where one is missing.
        MISSING_TEXTURE = id!(ns::MINECRAFT, "missingno"),
        /// What a book is given to players as.
        WRITTEN_BOOK = id!(ns::MINECRAFT, "written_book"),
        BOOK = id!(ns::MINECRAFT, "book"),
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

    /// What a mod asks of this client (`client_op`, ABI 3.9) — what the
    /// local player is and sees, and what it does: arguments and answers are JSON.
    client_op {
        /// Where it is and how it's doing: position, look, motion, health, food, dimension, slot.
        STATE = "state",
        /// Moves as if keys were held — `forward`, `strafe` in -1..1, `jump`, `sneak`, `sprint` — till `release`.
        DRIVE = "drive",
        /// Hands the movement keys back to the player.
        RELEASE = "release",
        /// Turns to `yaw`, `pitch` (degrees).
        LOOK = "look",
        /// Attacks entity `entity` (its UUID), or what the crosshair is on.
        ATTACK = "attack",
        /// Uses the held item (`off_hand` for the other hand).
        USE = "use",
        /// Keeps breaking the block at `x y z` from `face`; `stop` to give up.
        MINE = "mine",
        /// Places the held block against the block at `x y z`, on `face`.
        PLACE = "place",
        /// Selects hotbar slot `slot` (0–8).
        SLOT = "slot",
        /// The blocks among `ids` between `min` and `max`: `[x, y, z, id]` each.
        SCAN = "scan",
        /// The blocks between `min` and `max` for finding a way: a palette
        /// (`id`, `solid`, `liquid`, `climbable`) and an index per block, x fastest.
        AREA = "area",
        /// The entities within `radius`: `uuid`, `type`, position, health, a player's `name`.
        ENTITIES = "entities",
        /// The player's inventory: `slot`, `item`, `count` each.
        INVENTORY = "inventory",
        /// Clicks slot `slot` of the open container (`button`, `mode` as the game's).
        CLICK = "click",
        /// Sends `text` to the chat (a command when it starts with `/`).
        CHAT = "chat",
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
