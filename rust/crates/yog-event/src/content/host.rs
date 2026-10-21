//! What the hosts register, as the runtime hands it to them in one blob:
//! items and blocks with what they name resolved (materials and sound
//! groups whole), inventories laid out, recipes, dimensions, fonts, and
//! the mods' entity types, attributes, effects and potions as declared.
//! The hosts' Java reads it through the records generated from these; its
//! counts are `i32`, the `int`s the game's Java takes them as.

use yog_data::Schema;
use yog_data::Value;
use yog_data::with::Raw;
use yog_registry::{
    ArmorMaterial, ArmorSlot, AttributeDef, BiomeFeature, BlockProperty, EffectDef, EntityDef,
    Facing, FoodDef, GameTool, Placement, PotionDef, SoundGroup, ToolMaterial,
};

/// Everything the hosts register.
#[derive(Debug, Clone, Default, Schema)]
pub struct Content {
    pub items: Vec<Item>,
    /// Blocks made from another (stairs from planks) after the ones they're made from.
    pub blocks: Vec<Block>,
    pub inventories: Vec<Inventory>,
    pub recipes: Vec<Recipe>,
    /// For the hosts that add features to biomes in code (Fabric).
    pub biome_features: Vec<BiomeFeature>,
    pub entities: Vec<EntityDef>,
    pub dimensions: Vec<Dimension>,
    pub fonts: Vec<Font>,
    pub attributes: Vec<AttributeDef>,
    pub effects: Vec<EffectDef>,
    pub potions: Vec<PotionDef>,
}

/// An item: its stack, durability, fuel, food, tool or armor (materials
/// whole), what it's worn as or hangs, and whether it opens a book.
#[derive(Debug, Clone, Schema)]
pub struct Item {
    pub id: String,
    pub max_stack: i32,
    /// Empty: the game's name of its id.
    pub name: String,
    /// Empty: none.
    pub tooltip: String,
    pub max_damage: i32,
    pub fire_resistant: bool,
    pub fuel_ticks: i32,
    pub food: Option<FoodDef>,
    /// A book registered under the item's id: the item opens it.
    pub book: bool,
    pub tool: Option<Tool>,
    pub armor: Option<Armor>,
    /// The slot it's worn in without being armor.
    pub wearable: Option<ArmorSlot>,
    pub hangs: Option<Hangs>,
}

/// A tool: its kind (`pickaxe`…, or a mineable tag id), its material, and
/// its own attack where it gives one — else the game's tool it attacks as
/// (`game_tool`), which the host reads the game's numbers off.
#[derive(Debug, Clone, Schema)]
pub struct Tool {
    pub kind: String,
    /// On top of the material's.
    pub attack_damage: Option<f32>,
    pub attack_speed: Option<f32>,
    pub material: ToolMaterial,
    pub game_tool: Option<GameTool>,
}

/// A piece of armor: its slot and material, whole.
#[derive(Debug, Clone, Schema)]
pub struct Armor {
    pub slot: ArmorSlot,
    pub material: ArmorMaterial,
}

/// What an item hangs on a block's side: an entity of a hanging type,
/// `width`×`height` blocks.
#[derive(Debug, Clone, Schema)]
pub struct Hangs {
    pub entity: String,
    pub width: i32,
    pub height: i32,
}

/// A block: its properties, shape, sound group (whole), states, kind.
#[derive(Debug, Clone, Schema)]
pub struct Block {
    pub id: String,
    pub hardness: f32,
    pub resistance: f32,
    /// Empty: the game's name of its id.
    pub name: String,
    /// `x1, y1, z1, x2, y2, z2` in pixels.
    pub shape: Option<Vec<f32>>,
    pub light: i32,
    pub sound: Option<SoundGroup>,
    pub requires_tool: bool,
    pub no_collision: bool,
    pub slipperiness: f32,
    pub connects: bool,
    pub connect_groups: Vec<String>,
    /// The inventory it holds, by id.
    pub inventory: Option<String>,
    /// It's a redstone source.
    pub redstone: bool,
    /// Which ways it turns; `None`: it doesn't.
    pub facing: Option<Facing>,
    pub placement: Placement,
    pub attach: bool,
    pub waterlogged: bool,
    /// Its own properties: their values, the first the default.
    pub properties: Vec<BlockProperty>,
    pub powered: Option<String>,
    pub comparator: Option<String>,
    pub accepts_items: bool,
    pub no_occlusion: bool,
    /// One of the game's kinds of block (`stairs`, `door`…) or empty.
    pub kind: String,
    /// The block it's made from.
    pub base: Option<String>,
    /// A block set or wood type by name.
    pub set: Option<String>,
    pub press_ticks: i32,
    pub mobs_only: bool,
}

/// An inventory: its slots where its layout puts them, its background,
/// and — a machine's — its ingredients' and result's slots.
#[derive(Debug, Clone, Schema)]
pub struct Inventory {
    pub id: String,
    pub slot_count: i32,
    pub slots: Vec<InventorySlot>,
    pub include_player_inventory: bool,
    /// Where the player's inventory goes: `x, y`.
    pub player_inv: Vec<f32>,
    pub background_texture: Option<String>,
    pub title: String,
    /// `width, height`.
    pub background_size: Option<Vec<f32>>,
    pub machine: Option<Machine>,
}

/// A slot: where it is (`x, y`), and the item ids and `#tags` it takes (none: anything).
#[derive(Debug, Clone, Schema)]
pub struct InventorySlot {
    pub at: Vec<f32>,
    pub accepts: Vec<String>,
}

/// A machine's ingredients' slots and its result's (the recipe is the runtime's to find).
#[derive(Debug, Clone, Schema)]
pub struct Machine {
    pub places: Vec<i32>,
    pub result: i32,
}

/// A recipe in Yog's own format.
#[derive(Debug, Clone, Schema)]
pub struct Recipe {
    pub namespace: String,
    pub name: String,
    pub recipe: Value,
}

/// A dimension: its id and definition (`yog-dimensions`).
#[derive(Debug, Clone, Schema)]
pub struct Dimension {
    pub id: String,
    pub def: Value,
}

/// A Yog font as the game's text takes it: one cell per character, `rows`
/// of them in the PNG, its baseline `ascent` GUI pixels down a cell `height` high.
#[derive(Debug, Clone, Schema)]
pub struct Font {
    pub id: String,
    #[data(with = "Raw")]
    pub png: Vec<u8>,
    /// Each row's characters, cell by cell.
    pub rows: Vec<String>,
    /// GUI pixels from the top of a cell to the baseline, and a cell's height.
    pub ascent: i32,
    pub height: i32,
}
