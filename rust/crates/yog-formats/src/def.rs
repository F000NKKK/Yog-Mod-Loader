//! `.ydef` — definitions: an entry of some kind (`ladder`, `level`,
//! `sound_group`, …), named elsewhere by reference (`@<kind>:<id>`). No
//! vocabulary is a closed list: the game's values are built-in definitions
//! of the same kinds.

use yog_data::Schema;
use yog_data::Ordered;
use yog_data::with::{Opt, RefTo};
pub use yog_registry::{ArmorMaterial, SoundGroup, ToolMaterial};

yog_data::kind! {
    LadderKind = "ladder",
    LevelKind = "level",
    SoundGroupKind = "sound_group",
    ToolMaterialKind = "tool_material",
    ArmorMaterialKind = "armor_material",
}

/// A `.ydef`: an entry of some kind of definition, named by reference
/// (`@<kind>:<id>`) everywhere else. Kinds: `ladder` (an ordered scale),
/// `level` (a step on one), `sound_group` (what a block sounds like),
/// `recipe_kind` (a mod's kind of recipe), `recipe_layout` (how books
/// show the game's or another mod's), `tool_material` and
/// `armor_material`.
#[derive(Schema, Debug, Clone)]
#[data(tag = "kind")]
pub enum Def {
    /// An ordered scale — tool tiers (`minecraft:tool`, built in), a mod's
    /// voltages, rarities, magic circles…
    Ladder { id: String },
    /// A step on a ladder, just above another (the bottom if none).
    Level {
        id: String,
        #[data(with = "RefTo<LadderKind>")]
        ladder: String,
        #[data(with = "Opt<RefTo<LevelKind>>")]
        above: Option<String>,
    },
    /// What blocks naming it (`sound: "@sound_group:mymod:crystal"`) sound
    /// like: five sound events and a volume and pitch. The game's groups
    /// are built-in ones (`stone`, `amethyst`…).
    SoundGroup(SoundGroup),
    /// A mod's own kind of recipe (`mymod:infusion`): its slots and values,
    /// checked in every recipe of the kind, and how books show it.
    RecipeKind(RecipeKindDef),
    /// How books show kinds of recipe that aren't the mod's own — the
    /// game's (`shaped`), another mod's.
    RecipeLayout(yog_book::layout::RecipeLayout),
    /// What tools are made of (`@tool_material:mymod:mythril`); the game's
    /// are built in (`iron`, `diamond`…).
    ToolMaterial(ToolMaterialDef),
    /// What armor is made of (`@armor_material:mymod:mythril`); the game's
    /// are built in (`iron`, `diamond`…).
    ArmorMaterial(ArmorMaterialDef),
}

/// `{ kind: "tool_material", id: "mythril", durability: 1800, speed: 10,
/// attack_damage: 3, enchantability: 18, level: "@level:mymod:mythril",
/// repair: "@item:mymod:mythril_ingot" }`.
#[derive(Schema, Debug, Clone)]
pub struct ToolMaterialDef {
    pub id: String,
    /// Uses before it breaks.
    #[data(range = "1..=1000000")]
    pub durability: u32,
    /// How fast it mines (wood 2, stone 4, iron 6, diamond 8, netherite 9, gold 12).
    pub speed: f32,
    /// Attack damage it adds (wood 0, stone 1, iron 2, diamond 3, netherite 4).
    pub attack_damage: f32,
    #[data(range = "0..=1000")]
    pub enchantability: u32,
    /// Its level on the tool ladder: the game's (`iron`) or a mod's (`@level:mymod:mythril`).
    #[data(with = "crate::content::LevelRef")]
    pub level: String,
    /// What repairs it: an item or a tag of items.
    pub repair: yog_data::ItemIdOrTag,
}

/// `{ kind: "armor_material", id: "mythril", durability: 30, defense:
/// { head: 3, chest: 8, legs: 6, feet: 3 }, enchantability: 18,
/// equip_sound: "minecraft:item.armor.equip_diamond", repair: "@item:mymod:mythril_ingot" }`.
#[derive(Schema, Debug, Clone)]
pub struct ArmorMaterialDef {
    pub id: String,
    /// Multiplies each piece's base durability (helmet 11, chestplate 16, leggings 15, boots 13).
    #[data(range = "1..=10000")]
    pub durability: u32,
    pub defense: yog_registry::ArmorDefense,
    #[data(range = "0..=1000")]
    pub enchantability: u32,
    /// The sound event of putting it on (`minecraft:item.armor.equip_iron`).
    pub equip_sound: String,
    #[data(default)]
    pub toughness: f32,
    #[data(default, range = "0..=1")]
    pub knockback_resistance: f32,
    /// What repairs it: an item or a tag of items.
    pub repair: yog_data::ItemIdOrTag,
    /// Its look on a player (`mymod:mythril`: `mymod:textures/models/armor/mythril_layer_1.png`);
    /// its id by default.
    pub texture: Option<String>,
}

/// A slot of a kind of recipe: how many ingredients it takes.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct SlotSpec {
    /// At most this many (each one item or a list of alternatives).
    #[data(default = "one", range = "1..=64")]
    pub max: u32,
    /// A recipe of the kind must fill it.
    #[data(default)]
    pub required: bool,
}

fn one() -> u32 {
    1
}

/// What a recipe's value is.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Number,
    Text,
    Bool,
}

/// A mod's kind of recipe: `{ kind: "recipe_kind", id: "infusion", slots:
/// { core: { required: true }, ring: { max: 8 } }, values: { time:
/// "number" }, layout: { size: [96, 24], parts: [ … ] } }`.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct RecipeKindDef {
    /// `infusion` (the mod's namespace is added) or `mymod:infusion`.
    pub id: String,
    #[data(default)]
    pub slots: Ordered<SlotSpec>,
    #[data(default)]
    pub values: Ordered<ValueType>,
    /// How books show its recipes (`yog_book::layout`): none, and a book
    /// can't show them.
    pub layout: Option<KindLayout>,
}

/// A [`yog_book::layout::RecipeLayout`] of one kind, the kind's own.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct KindLayout {
    /// Width and height in GUI pixels.
    pub size: [f32; 2],
    #[data(default)]
    pub parts: Vec<yog_book::layout::LayoutPart>,
}

/// The game's tool tiers: `@ladder:minecraft:tool`, bottom to top (gold mines like wood).
pub const TOOL_LADDER: &str = "minecraft:tool";
pub const TOOL_LEVELS: [&str; 5] = ["wood", "stone", "iron", "diamond", "netherite"];
