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

    /// The bus's events (`docs/design/bus.md`): `yog:event/<plane>/<name>`
    /// after the game did it, `…/pre` before (a cancel stops it).
    event {
        /// What every event address starts with.
        PREFIX = concatcp!(ns::YOG, sep::NAMESPACE, "event", sep::PATH),
        /// What a pre-event's address ends with.
        PRE = concatcp!(sep::PATH, "pre"),
    }

    /// `yog:event/player/session/…`.
    event_player_session {
        PLANE = concatcp!(event::PREFIX, "player", sep::PATH, "session", sep::PATH),
        JOIN = concatcp!(PLANE, "join"),
        LEAVE = concatcp!(PLANE, "leave"),
    }

    /// `yog:event/player/life/…`.
    event_player_life {
        PLANE = concatcp!(event::PREFIX, "player", sep::PATH, "life", sep::PATH),
        DEATH = concatcp!(PLANE, "death"),
        DEATH_PRE = concatcp!(DEATH, event::PRE),
        RESPAWN = concatcp!(PLANE, "respawn"),
    }

    /// `yog:event/player/chat/…`.
    event_player_chat {
        PLANE = concatcp!(event::PREFIX, "player", sep::PATH, "chat", sep::PATH),
        MESSAGE = concatcp!(PLANE, "message"),
        MESSAGE_PRE = concatcp!(MESSAGE, event::PRE),
    }

    /// `yog:event/player/action/…`.
    event_player_action {
        PLANE = concatcp!(event::PREFIX, "player", sep::PATH, "action", sep::PATH),
        MOVE = concatcp!(PLANE, "move"),
        USE_ITEM = concatcp!(PLANE, "use_item"),
        USE_ITEM_PRE = concatcp!(USE_ITEM, event::PRE),
        ATTACK = concatcp!(PLANE, "attack"),
        INTERACT = concatcp!(PLANE, "interact"),
        INTERACT_PRE = concatcp!(INTERACT, event::PRE),
        CRAFT = concatcp!(PLANE, "craft"),
        PICKUP = concatcp!(PLANE, "pickup"),
        PICKUP_PRE = concatcp!(PICKUP, event::PRE),
    }

    /// `yog:event/player/progress/…`.
    event_player_progress {
        PLANE = concatcp!(event::PREFIX, "player", sep::PATH, "progress", sep::PATH),
        ADVANCEMENT = concatcp!(PLANE, "advancement"),
    }

    /// `yog:event/block/…`.
    event_block {
        PLANE = concatcp!(event::PREFIX, "block", sep::PATH),
        BREAK = concatcp!(PLANE, "break"),
        BREAK_PRE = concatcp!(BREAK, event::PRE),
        PLACE = concatcp!(PLANE, "place"),
        PLACE_PRE = concatcp!(PLACE, event::PRE),
        USE = concatcp!(PLANE, "use"),
        USE_PRE = concatcp!(USE, event::PRE),
        INSERT = concatcp!(PLANE, "insert"),
    }

    /// `yog:event/entity/life/…`.
    event_entity_life {
        PLANE = concatcp!(event::PREFIX, "entity", sep::PATH, "life", sep::PATH),
        SPAWN = concatcp!(PLANE, "spawn"),
        SPAWN_PRE = concatcp!(SPAWN, event::PRE),
        DEATH = concatcp!(PLANE, "death"),
    }

    /// `yog:event/entity/combat/…`.
    event_entity_combat {
        PLANE = concatcp!(event::PREFIX, "entity", sep::PATH, "combat", sep::PATH),
        DAMAGE = concatcp!(PLANE, "damage"),
        DAMAGE_PRE = concatcp!(DAMAGE, event::PRE),
        PROJECTILE_HIT = concatcp!(PLANE, "projectile_hit"),
        PROJECTILE_HIT_PRE = concatcp!(PROJECTILE_HIT, event::PRE),
    }

    /// `yog:event/world/…`.
    event_world {
        PLANE = concatcp!(event::PREFIX, "world", sep::PATH),
        EXPLOSION = concatcp!(PLANE, "explosion"),
        EXPLOSION_PRE = concatcp!(EXPLOSION, event::PRE),
    }

    /// `yog:event/container/…`.
    event_container {
        PLANE = concatcp!(event::PREFIX, "container", sep::PATH),
        OPEN = concatcp!(PLANE, "open"),
        OPEN_PRE = concatcp!(OPEN, event::PRE),
        CLOSE = concatcp!(PLANE, "close"),
    }

    /// `yog:event/server/game/…`.
    event_server_game {
        PLANE = concatcp!(event::PREFIX, "server", sep::PATH, "game", sep::PATH),
        TICK = concatcp!(PLANE, "tick"),
    }

    /// `yog:event/server/life/…`.
    event_server_life {
        PLANE = concatcp!(event::PREFIX, "server", sep::PATH, "life", sep::PATH),
        STARTED = concatcp!(PLANE, "started"),
        STOPPING = concatcp!(PLANE, "stopping"),
    }

    /// `yog:event/client/game/…`.
    event_client_game {
        PLANE = concatcp!(event::PREFIX, "client", sep::PATH, "game", sep::PATH),
        TICK = concatcp!(PLANE, "tick"),
    }

    /// `yog:event/client/input/…`.
    event_client_input {
        PLANE = concatcp!(event::PREFIX, "client", sep::PATH, "input", sep::PATH),
        KEY = concatcp!(PLANE, "key"),
        KEY_PRE = concatcp!(KEY, event::PRE),
    }

    /// `yog:event/client/ui/…`.
    event_client_ui {
        PLANE = concatcp!(event::PREFIX, "client", sep::PATH, "ui", sep::PATH),
        OPEN = concatcp!(PLANE, "open"),
        CLOSE = concatcp!(PLANE, "close"),
    }

    /// `yog:event/client/render/…`.
    event_client_render {
        PLANE = concatcp!(event::PREFIX, "client", sep::PATH, "render", sep::PATH),
        HUD = concatcp!(PLANE, "hud"),
    }


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
    /// A texture: its picture as given and how it animates.
    Texture = "ytexture", kind::TEXTURE;
    /// A material: the textures a surface is drawn with, and how.
    Material = "ymaterial", kind::MATERIAL;
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

/// Texture `path`'s file under its namespace: `block/stone` → `textures/block/stone.png`.
#[must_use]
pub fn texture_file(path: &str) -> String {
    format!("{}{path}{}", res::TEXTURES, res::PNG)
}

/// Model `path`'s file: `item/apple` → `models/item/apple.json`.
#[must_use]
pub fn model_file(path: &str) -> String {
    format!("{}{path}{}", res::MODELS, res::JSON)
}

/// Block `path`'s states' file: `stone` → `blockstates/stone.json`.
#[must_use]
pub fn blockstate_file(path: &str) -> String {
    format!("{}{path}{}", res::BLOCKSTATES, res::JSON)
}

/// The game's registry of content `kind` — what its tags' and data's
/// folders are named: `entity` → `entity_type`, `biome` → `worldgen/biome`.
#[must_use]
pub fn registry_of(kind: &str) -> &str {
    match kind {
        kind::ENTITY => "entity_type",
        kind::BIOME => "worldgen/biome",
        other => other,
    }
}

/// The kinds whose data and tag folders were plural before 1.21 (`items`, `loot_tables`).
const PLURAL_BEFORE_1_21: &[&str] = &[kind::ITEM, kind::BLOCK, kind::ENTITY, kind::FLUID, kind::LOOT_TABLE, kind::ADVANCEMENT, kind::RECIPE];

/// The folders of content `kind`'s data or tags, the current one first, then
/// 1.20's plural one when it was named differently: `item` → `item`, `items`.
#[must_use]
pub fn registry_dirs(kind: &str) -> Vec<String> {
    let dir = registry_of(kind);
    let mut out = vec![dir.to_string()];
    if PLURAL_BEFORE_1_21.contains(&kind) {
        out.push(format!("{dir}s"));
    }
    out
}

/// Language `locale`'s file under a namespace: `lang/en_us.json`.
#[must_use]
pub fn lang_file(locale: &str) -> String {
    format!("{}{locale}{}", res::LANG, res::JSON)
}

/// What the game's translation keys of content `kind` in namespace `ns` start with: `item.minecraft.`.
#[must_use]
pub fn lang_prefix(kind: &str, ns: &str) -> String {
    format!("{kind}.{ns}.")
}

/// The game's translation key of content `kind` `id`: `item.minecraft.iron_ingot`, a path's `/` as `.`.
#[must_use]
pub fn lang_key(kind: &str, id: &str) -> String {
    let (ns, path) = split(id);
    format!("{}{}", lang_prefix(kind, ns), path.replace(sep::PATH, "."))
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
        assert_eq!(registry_of(kind::BIOME), "worldgen/biome");
        assert_eq!(Format::Anim.file("walk"), "walk.yanim");
    }

    /// The hosts' copy (`java-common`), when this crate is in its repository.
    #[test]
    fn the_java_names_are_current() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../java-common/src/main/java/dev/yog/YogNames.java");
        let Ok(java) = std::fs::read_to_string(&file) else { return };
        assert!(
            java == export::java("dev.yog", "YogNames"),
            "{} is stale: cargo run -p yog-ids --bin yog-ids-export -- --java ../java-common/src/main/java/dev/yog dev.yog YogNames",
            file.display()
        );
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
