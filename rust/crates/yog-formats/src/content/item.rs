//! A `.yitem`'s properties: stacking, food, potions, tools and armor.

use yog_data::Schema;
use yog_data::with::RefTo;

use super::refs::{MaterialRef, ToolKindRef};
use crate::def::{ArmorMaterialKind, ToolMaterialKind};

/// What eating it gives.
#[derive(Schema, Debug, Clone)]
pub struct Food {
    #[data(range = "0..=1000")]
    pub nutrition: u32,
    #[data(default, range = "0..=100")]
    pub saturation: f64,
    /// Eaten even when not hungry.
    #[data(default)]
    pub always_edible: bool,
}

/// A potion type attached to the item file: the effects the bottle gives.
#[derive(Schema, Debug, Clone)]
pub struct PotionProps {
    /// Translation suffix shared by the bottle variants (optional).
    pub name: Option<String>,
    pub effects: Vec<yog_registry::PotionEffect>,
}

/// An item's properties.
#[derive(Schema, Debug, Clone, Default)]
pub struct ItemProps {
    /// How many stack (1–99; 1 with durability).
    #[data(range = "1..=99")]
    pub stack_size: Option<u8>,
    /// Uses before it breaks (0: none).
    #[data(range = "0..=1000000000")]
    pub durability: Option<u32>,
    /// Survives fire and lava.
    #[data(default)]
    pub fire_resistant: bool,
    /// Ticks it burns in a furnace.
    #[data(range = "0..=1000000000")]
    pub fuel_ticks: Option<u32>,
    pub food: Option<Food>,
    /// A tool or a weapon: its kind and material.
    pub tool: Option<ToolProps>,
    /// A piece of armor: its slot and material.
    pub armor: Option<ArmorProps>,
    /// Worn in this slot without being armor (a monocle, a crown):
    /// right-click and dispensers put it on.
    pub wearable: Option<ArmorSlot>,
    /// A potion's effects. Brewing recipes live in `.yrecipe` files.
    pub potion: Option<PotionProps>,
    /// Used on a block's side, it hangs there (a painting, a scroll on a wall).
    pub hangs: Option<HangsProps>,
}

/// What an item hangs on a wall: an entity of a hanging type that keeps one
/// of the item and drops it when it breaks.
#[derive(Schema, Debug, Clone)]
pub struct HangsProps {
    /// The entity (`@entity:mymod:wall_scroll`, a `.yentity` of `kind: hanging`).
    #[data(with = "RefTo<yog_data::kinds::Entity>")]
    pub entity: String,
    /// Whole blocks of wall, wide and high; default: the entity type's size.
    pub size: Option<[u32; 2]>,
}

/// A block's item form.
#[derive(Schema, Debug, Clone, Default)]
pub struct BlockItemProps {
    /// How many stack (1–99; 64 by default).
    #[data(range = "1..=99")]
    pub stack_size: Option<u8>,
    /// Survives fire and lava.
    #[data(default)]
    pub fire_resistant: bool,
    /// Ticks it burns in a furnace.
    #[data(range = "0..=1000000000")]
    pub fuel_ticks: Option<u32>,
}

/// An item as a tool or weapon.
#[derive(Schema, Debug, Clone)]
pub struct ToolProps {
    /// `pickaxe`, `axe`, `shovel`, `hoe`, `sword`, or a mod's kind of tool
    /// by the blocks it mines (`@tag:block:mymod:mineable/hammer`).
    #[data(with = "ToolKindRef")]
    pub kind: String,
    /// The game's material (`wood`, `stone`, `iron`, `diamond`, `gold`,
    /// `netherite`) or a mod's (`@tool_material:mymod:mythril`, a `.ydef`).
    #[data(with = "MaterialRef<ToolMaterialKind>")]
    pub material: String,
    /// Attack damage on top of the material's; by default the game's tool
    /// of the kind and material has (`yog_registry::ToolMaterial::game_tool`),
    /// as the running version of the game has it.
    pub attack_damage: Option<f32>,
    /// Attack speed change; by default the game's tool's.
    pub attack_speed: Option<f32>,
}

/// Where a piece of armor is worn.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorSlot {
    Head,
    Chest,
    Legs,
    Feet,
}

/// An item as a piece of armor.
#[derive(Schema, Debug, Clone)]
pub struct ArmorProps {
    pub slot: ArmorSlot,
    /// The game's material (`leather`, `chainmail`, `iron`, `gold`,
    /// `diamond`, `turtle`, `netherite`) or a mod's
    /// (`@armor_material:mymod:mythril`, a `.ydef`).
    #[data(with = "MaterialRef<ArmorMaterialKind>")]
    pub material: String,
}
