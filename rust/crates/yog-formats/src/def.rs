//! `.ydef` — definitions: an entry of some kind (`ladder`, `level`,
//! `sound_group`, …), named elsewhere by reference (`@<kind>:<id>`). No
//! vocabulary is a closed list: the game's values are built-in definitions
//! of the same kinds.

use yog_data::Ordered;
use yog_data::Schema;
use yog_data::with::{Opt, RefTo};
pub use yog_registry::{ArmorMaterial, SoundGroup, ToolMaterial};

yog_data::kind! {
    AdvancementKind = yog_ids::kind::ADVANCEMENT,
    LadderKind = yog_ids::kind::LADDER,
    LevelKind = yog_ids::kind::LEVEL,
    SoundGroupKind = yog_ids::kind::SOUND_GROUP,
    ToolMaterialKind = yog_ids::kind::TOOL_MATERIAL,
    ArmorMaterialKind = yog_ids::kind::ARMOR_MATERIAL,
}

/// A `.ydef`: an entry of some kind of definition, named by reference
/// (`@<kind>:<id>`) everywhere else. Kinds: `ladder` (an ordered scale),
/// `level` (a step on one), `sound_group` (what a block sounds like),
/// `recipe_kind` (a mod's kind of recipe; how it's shown is a `.yui`
/// `recipe_layout`, see [`crate::book::layout`]), `tool_material`,
/// `armor_material`, `ore`, `dimension`, `table`, `tag` (content of one
/// kind grouped under a name) and `damage_type`.
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
    /// checked in every recipe of the kind.
    RecipeKind(RecipeKindDef),
    /// What tools are made of (`@tool_material:mymod:mythril`); the game's
    /// are built in (`iron`, `diamond`…).
    ToolMaterial(ToolMaterialDef),
    /// What armor is made of (`@armor_material:mymod:mythril`); the game's
    /// are built in (`iron`, `diamond`…).
    ArmorMaterial(ArmorMaterialDef),
    /// An ore in the world: veins of the mod's blocks in place of stone,
    /// in the biomes it says — the overworld's or any dimension's.
    Ore(OreDef),
    /// A dimension: its type (sky, light, height…), its biome, and its
    /// ground as data (`terrain`, see `yog_worldgen`) — or a generator the
    /// mod's code registers (`generator`).
    Dimension(DimensionDef),
    /// A mod's own table — tiers, materials, voltages…: rows of the columns
    /// it declares, which `yog build` makes an enum in the mod's code.
    Table(TableDef),
    /// A tag: content of one kind under a name (`hexcasting:water_plants`,
    /// blocks), or more of it added to another's (`minecraft:logs`).
    Tag(TagDef),
    /// A kind of damage (`hexcasting:overcast`): how it's scaled, what it
    /// exhausts, its death message, the tags it's in (`minecraft:bypasses_armor`).
    DamageType(DamageTypeDef),
    /// A tree the world (or a sapling, a spell) grows: its trunk and leaves.
    Tree(TreeDef),
    /// Changes to the loot of some tables (chests, block drops, mobs):
    /// items added — how many, at what chance, finished by a loot function
    /// of the mod's code — and items scaled.
    LootModifier(yog_registry::LootModifier),
    /// A loot table of the mod's own (`hexcasting:random_scroll`, for `/loot`
    /// and other tables to roll): what it gives, as a loot modifier's `add`.
    LootTable(LootTableDef),
    /// A number every player has, which effects change
    /// (`hexcasting:grid_zoom`).
    Attribute(yog_registry::AttributeDef),
    /// A mob effect: good or bad, its color, the attributes it changes.
    Effect(yog_registry::EffectDef),
    /// An advancement: where it hangs, how it looks, what earns it.
    Advancement(AdvancementDef),
}

/// `{ kind: "advancement", id: "lore/cardamom1", parent: "@advancement:hexcasting:lore",
/// icon: "@item:hexcasting:lore_fragment", title: "@Lang.…", hidden: true,
/// criteria: [{ granted: true }] }` — the game's advancement, its JSON for
/// every version.
#[derive(Schema, Debug, Clone)]
pub struct AdvancementDef {
    /// `lore/cardamom1` (the mod's).
    pub id: String,
    /// The one it hangs under; a tab's root has none.
    #[data(with = "Opt<RefTo<AdvancementKind>>")]
    pub parent: Option<String>,
    #[data(with = "RefTo<yog_data::kinds::Item>")]
    pub icon: String,
    /// A `.yres` text (`@Lang.…`), a translation key or words.
    pub title: yog_data::Text,
    pub description: Option<yog_data::Text>,
    /// `task` (default), `goal`, `challenge`.
    #[data(default = "task_frame")]
    pub frame: String,
    /// Not shown before it's had.
    #[data(default)]
    pub hidden: bool,
    /// A root's background texture (`minecraft:textures/block/calcite.png`).
    pub background: Option<String>,
    /// What earns it — every one of them.
    pub criteria: Vec<CriterionDef>,
}

fn task_frame() -> String {
    "task".into()
}

/// One way to earn an advancement — one of: `granted` (the mod's code
/// awards it), `has_item` (an item or a tag of them in the inventory),
/// `used_item` (eaten, drunk, used up).
#[derive(Schema, Debug, Clone, Default)]
pub struct CriterionDef {
    #[data(default)]
    pub granted: bool,
    pub has_item: Option<yog_data::ItemIdOrTag>,
    pub used_item: Option<yog_data::ItemId>,
}

/// `{ kind: "loot_table", id: "random_scroll", add: [{ item: "@item:…",
/// function: "mymod:…" }] }`.
#[derive(Schema, Debug, Clone)]
pub struct LootTableDef {
    /// `random_scroll` (the mod's).
    pub id: String,
    pub add: Vec<yog_registry::LootAdd>,
}

/// `{ kind: "tree", id: "amethyst_edified_tree", trunk: { blocks: [{ block:
/// "@block:hexcasting:edified_log", weight: 8 }, …], shape: "fancy",
/// base_height: 5, random_a: 5, random_b: 3 }, leaves: { blocks: [{ block:
/// "@block:hexcasting:amethyst_edified_leaves" }], shape: "fancy", radius:
/// 1, offset: 5, height: 5 } }` — the game's `minecraft:tree` feature.
#[derive(Schema, Debug, Clone)]
pub struct TreeDef {
    /// `amethyst_edified_tree` (the mod's).
    pub id: String,
    pub trunk: TreeTrunk,
    pub leaves: TreeLeaves,
    /// What the ground under it turns into (default dirt).
    pub dirt: Option<yog_data::BlockId>,
    /// It grows no shorter than this, clipping into what's above (the
    /// game's `min_clipped_height`).
    pub min_clipped_height: Option<u32>,
    #[data(default)]
    pub ignore_vines: bool,
    /// The ground turns to `dirt` even where it's grass.
    #[data(default)]
    pub force_dirt: bool,
}

/// A trunk: its blocks, how it's shaped and how tall.
#[derive(Schema, Debug, Clone)]
pub struct TreeTrunk {
    pub blocks: Vec<WeightedBlock>,
    /// `straight`, `forking`, `giant`, `mega_jungle`, `dark_oak`, `fancy`,
    /// `bending`, `upwards_branching`, `cherry` (the game's) or a mod's
    /// trunk placer by id.
    pub shape: String,
    pub base_height: u32,
    /// Random height added: `0..=random_a`, then `0..=random_b`.
    #[data(default)]
    pub random_a: u32,
    #[data(default)]
    pub random_b: u32,
}

/// Leaves: their blocks and the shape they're placed in.
#[derive(Schema, Debug, Clone)]
pub struct TreeLeaves {
    pub blocks: Vec<WeightedBlock>,
    /// `blob`, `spruce`, `pine`, `acacia`, `bush`, `fancy`, `jungle`,
    /// `mega_pine`, `dark_oak`, `random_spread`, `cherry` (the game's) or a
    /// mod's foliage placer by id.
    pub shape: String,
    pub radius: u32,
    #[data(default)]
    pub offset: i32,
    /// How many layers (the shapes that have it: `blob`, `fancy`, `bush`, `jungle`).
    pub height: Option<u32>,
}

/// A block, picked `weight` times as often as a weight-1 one.
#[derive(Schema, Debug, Clone)]
pub struct WeightedBlock {
    pub block: yog_data::BlockId,
    #[data(default = "one")]
    pub weight: u32,
}

/// `{ kind: "tag", of: "entity", id: "sticky_teleporters", values:
/// ["@entity:minecraft:pig", "@tag:entity:c:bosses"], optional:
/// ["@entity:create:seat"] }` — written for each game version the way it
/// spells tags (`tags/entity_type/`, `tags/entity_types/`).
#[derive(Schema, Debug, Clone)]
pub struct TagDef {
    /// The kind of content: `item`, `block`, `entity`, `fluid`, `biome`,
    /// `damage_type` — or any of the game's registries by its name.
    pub of: String,
    /// `water_plants` (the mod's) or another's to add to (`minecraft:logs`).
    pub id: String,
    /// What's in it: `@<kind>:<id>` — checked to exist — or `@tag:<kind>:<id>`.
    #[data(default)]
    pub values: Vec<yog_data::AnyRefOrTag>,
    /// What's in it if it's there — another mod's content
    /// (`@entity:create:seat`), not checked.
    #[data(default)]
    pub optional: Vec<yog_data::AnyRefOrTag>,
}

fn scaled_by_mobs() -> String {
    "when_caused_by_living_non_player".into()
}

/// `{ kind: "damage_type", id: "overcast", scaling: "never", exhaustion:
/// 0, tags: ["minecraft:bypasses_armor"] }`.
#[derive(Schema, Debug, Clone)]
pub struct DamageTypeDef {
    /// `overcast` (the mod's).
    pub id: String,
    /// Whether difficulty scales it: `never`, `always`,
    /// `when_caused_by_living_non_player` (the game's default).
    #[data(default = "scaled_by_mobs")]
    pub scaling: String,
    /// Hunger it costs.
    #[data(default)]
    pub exhaustion: f32,
    /// Its death message is `death.attack.<message_id>` (default `<ns>.<id>`).
    pub message_id: Option<String>,
    /// The sound when it hurts: `hurt` (default), `thorns`, `drowning`,
    /// `burning`, `poking`, `freezing`.
    pub effects: Option<String>,
    /// How its death message reads: `default`, `fall_variants`,
    /// `intentional_game_design`.
    pub death_message_type: Option<String>,
    /// Damage type tags it's in (`minecraft:bypasses_armor`,
    /// `minecraft:bypasses_effects`, a mod's).
    #[data(default)]
    pub tags: Vec<String>,
}

/// `{ kind: "table", id: "tier", columns: { size: "u32", chip: "item",
/// name: "text" }, rows: { wood: { size: 16, chip: "@item:mymod:chip_wood",
/// name: "@Strings.tier_wood" }, … } }` — in the mod's code an enum `Tier`,
/// a variant per row (`Tier::Wood`, in order), a method per column
/// (`Tier::Wood.size()`), `Tier::ALL`, `id()`, `from_id()` and, for an
/// `item`, `block` or `string` column, `from_<column>()`.
///
/// A column is `bool`, an integer (`u8` … `u64`, `i32`, `i64`, `usize`),
/// `f32`, `f64`, `string`, `text` (a `.yres` string, translated), `color`
/// (a `.yres` color), `item` or `block` (`@item:`/`@block:`), a list of one
/// of those but text (`[item]`) or a map of them (`{item: u64}`, in order).
#[derive(Schema, Debug, Clone)]
pub struct TableDef {
    /// `tier`: the enum is `Tier`.
    pub id: String,
    pub columns: Ordered<String>,
    pub rows: Ordered<yog_data::Value>,
    /// The enum (de)serializes with serde (`Wood` as `"Wood"`): the mod
    /// depends on serde.
    #[data(default)]
    pub serde: bool,
}

/// `{ kind: "dimension", id: "crystal_caves", dimension_type: { has_sky_light:
/// false }, biome: "@biome:minecraft:plains", terrain: { … } }`.
#[derive(Schema, Debug, Clone)]
pub struct DimensionDef {
    /// `crystal_caves` (the mod's), or the game's to change it (`minecraft:overworld`).
    pub id: String,
    #[data(default)]
    pub dimension_type: DimensionTypeProps,
    /// The biome it's made of (default `minecraft:plains`).
    pub biome: Option<yog_data::Id<yog_data::kinds::Biome>>,
    /// Structures (villages…) may start in it.
    #[data(default = "yes")]
    pub structures: bool,
    /// The biome's features (trees, flowers, its ores) are placed.
    #[data(default = "yes")]
    pub features: bool,
    #[data(default = "yes")]
    pub mob_spawning: bool,
    /// Its ground, as data.
    pub terrain: Option<yog_worldgen::Terrain>,
    /// Or a generator the mod's code registers (`Registry::register_chunk_generator`).
    pub generator: Option<String>,
}

fn yes() -> bool {
    true
}

/// A dimension type's properties; what's left out is the overworld's.
#[derive(Schema, Debug, Clone, Default)]
pub struct DimensionTypeProps {
    pub min_y: Option<i32>,
    pub height: Option<i32>,
    pub logical_height: Option<i32>,
    pub has_sky_light: Option<bool>,
    pub has_ceiling: Option<bool>,
    #[data(range = "0..=1")]
    pub ambient_light: Option<f32>,
    pub coordinate_scale: Option<f64>,
    pub ultrawarm: Option<bool>,
    pub natural: Option<bool>,
    pub beds_explode: Option<bool>,
    pub respawn_anchors_explode: Option<bool>,
    pub piglin_safe: Option<bool>,
    /// The sky and fog the game draws: `minecraft:overworld`, `minecraft:the_nether`, `minecraft:the_end`.
    pub effects: Option<String>,
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
    ("netherrack", yog_ids::id!(yog_ids::ns::MINECRAFT, "netherrack")),
    ("end_stone", yog_ids::id!(yog_ids::ns::MINECRAFT, "end_stone")),
];

impl yog_data::with::With<String> for OreReplaces {
    fn shape() -> yog_data::Shape {
        yog_data::Shape::RefOrTag(&[yog_ids::kind::BLOCK])
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
            Some(t) => yog_data::Value::Tag(yog_data::Ref::new(yog_ids::kind::BLOCK, t)),
            None => yog_data::Value::Ref(yog_data::Ref::new(yog_ids::kind::BLOCK, f.clone())),
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

/// Kinds of recipe are the registry's: the game's built in, a mod's here.
pub use yog_registry::recipe_kinds::{RecipeKindDef, SlotSpec, ValueType};

fn one() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_kind_of_definition_is_a_kind_of_content() {
        use yog_data::{Schema, Shape};
        let Shape::Enum(e) = super::Def::shape() else {
            panic!("a definition is an enum by its kind")
        };
        let kinds = yog_ids::groups()
            .find(|(g, _)| *g == "kind")
            .expect("yog-ids has kinds")
            .1;
        for v in &e.variants {
            assert!(
                kinds.iter().any(|(_, k)| *k == v.name),
                "`{}` isn't in yog_ids::kind",
                v.name
            );
        }
    }
}
