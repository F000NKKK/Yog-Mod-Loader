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
    /// An ore in the world: veins of the mod's blocks in place of stone,
    /// in the biomes it says — the overworld's or any dimension's.
    Ore(OreDef),
}

/// `{ kind: "ore", id: "mythril_ore", targets: [{ replaces: "stone", block:
/// "@block:mymod:mythril_ore" }, { replaces: "deepslate", block:
/// "@block:mymod:deepslate_mythril_ore" }], size: 6, per_chunk: 4, height:
/// { min: -48, max: 16, shape: "triangle" } }`.
#[derive(Schema, Debug, Clone)]
pub struct OreDef {
    pub id: String,
    /// What it replaces with what: one ore block for each kind of stone.
    pub targets: Vec<OreTarget>,
    /// Blocks in a vein, at most.
    #[data(range = "1..=64")]
    pub size: u32,
    /// Veins a chunk.
    #[data(range = "0..=256")]
    pub per_chunk: u32,
    pub height: OreHeight,
    /// The chance that a block of a vein touching air isn't placed (0–1;
    /// the game's diamonds: 0.5 for small veins).
    #[data(default, range = "0..=1")]
    pub air_exposure: f32,
    /// Where: a tag of biomes — the overworld's by default.
    pub biomes: Option<yog_data::TagId<yog_data::kinds::Biome>>,
}

/// One kind of stone an ore replaces, and the block that takes its place.
#[derive(Schema, Debug, Clone)]
pub struct OreTarget {
    /// `stone` (the overworld's stones), `deepslate`, `netherrack`,
    /// `end_stone`, a tag of blocks (`@tag:block:mymod:marble`) or a block
    /// (`@block:minecraft:basalt`).
    #[data(with = "OreReplaces")]
    pub replaces: String,
    pub block: yog_data::BlockId,
}

/// Where a vein may be: `min` to `max` blocks high, evenly (`uniform`) or
/// mostly in the middle (`triangle`).
#[derive(Schema, Debug, Clone)]
pub struct OreHeight {
    pub min: i32,
    pub max: i32,
    #[data(default)]
    pub shape: HeightShape,
}

#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeightShape {
    #[default]
    Uniform,
    Triangle,
}

/// What an ore replaces: `#tag` or a block id, as the game's ore targets write it.
pub struct OreReplaces;

/// The game's stones by short name: what their ores replace.
pub const ORE_STONES: [(&str, &str); 4] = [
    ("stone", "#minecraft:stone_ore_replaceables"),
    ("deepslate", "#minecraft:deepslate_ore_replaceables"),
    ("netherrack", "minecraft:netherrack"),
    ("end_stone", "minecraft:end_stone"),
];

impl yog_data::with::With<String> for OreReplaces {
    fn shape() -> yog_data::Shape {
        yog_data::Shape::RefOrTag(&["block"])
    }
    fn read(v: &yog_data::ValueRef<'_>, cx: &mut yog_data::Reader) -> Option<String> {
        use yog_data::ValueRef;
        match v {
            ValueRef::Str(s) if !s.starts_with('@') => ORE_STONES.iter().find(|(n, _)| n == s).map(|(_, t)| t.to_string()).or_else(|| {
                let names: Vec<&str> = ORE_STONES.iter().map(|(n, _)| *n).collect();
                cx.problem(format!("`{s}`: one of {} — or a tag of blocks (`@tag:block:…`) or a block (`@block:…`)", names.join(", ")));
                None
            }),
            ValueRef::Tag { .. } => yog_data::TagId::<yog_data::kinds::Block>::read(v, cx).map(|t| format!("#{}", t.id)),
            ValueRef::Str(s) if s.starts_with("@tag:") => yog_data::TagId::<yog_data::kinds::Block>::read(v, cx).map(|t| format!("#{}", t.id)),
            _ => yog_data::with::RefTo::<yog_data::kinds::Block>::read(v, cx),
        }
    }
    fn write(f: &String) -> yog_data::Value {
        if let Some((n, _)) = ORE_STONES.iter().find(|(_, t)| t == f) {
            return yog_data::Value::from(*n);
        }
        match f.strip_prefix('#') {
            Some(t) => yog_data::Value::Tag(yog_data::Ref::new("block", t)),
            None => yog_data::Value::Ref(yog_data::Ref::new("block", f.clone())),
        }
    }
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
