//! Items: what a mod declares an item as (food, hangings, its look).

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{ArmorPiece, ArmorSlot, ToolItem};

/// Nutritional properties for a food item.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone)]
pub struct FoodDef {
    /// Hunger points restored (1 unit = half a drumstick).
    pub nutrition: u32,
    /// Saturation modifier applied after eating.
    pub saturation: f32,
    /// If `true`, edible even when the hunger bar is full.
    pub can_always_eat: bool,
}

impl FoodDef {
    pub fn new(nutrition: u32, saturation: f32) -> Self {
        Self {
            nutrition,
            saturation,
            can_always_eat: false,
        }
    }

    pub fn can_always_eat(mut self) -> Self {
        self.can_always_eat = true;
        self
    }
}

/// A custom item to register, identified by `namespace:path`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone)]
pub struct ItemDef {
    pub id: String,
    pub max_stack: u8,
    pub name: Option<String>,
    pub tooltip: Option<String>,
    /// Durability. `0` = non-damageable. If set, `max_stack` is forced to 1.
    pub max_damage: u32,
    /// Immune to fire and lava damage (like netherite items).
    pub fire_resistant: bool,
    /// Furnace fuel burn time in ticks (`0` = not fuel; 200 = one coal equivalent).
    pub fuel_ticks: u32,
    /// Nutritional properties; `None` = not food.
    pub food: Option<FoodDef>,
    /// A tool or a weapon of a material.
    pub tool: Option<ToolItem>,
    /// A piece of armor of a material.
    pub armor: Option<ArmorPiece>,
    /// Worn in this slot without being armor.
    pub wearable: Option<ArmorSlot>,
    /// Used on a block's side, it hangs there as an entity of a hanging type.
    pub hangs: Option<Hangs>,
}

/// What an item hangs on a wall ([`ItemDef::hangs`]).
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(yog_data::Schema, Debug, Clone, PartialEq)]
pub struct Hangs {
    /// A hanging entity type ([`EntityKind::Hanging`]).
    pub entity: String,
    /// Whole blocks of wall, wide and high; `None`: the type's size.
    pub size: Option<[u32; 2]>,
}

impl ItemDef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            max_stack: 64,
            name: None,
            tooltip: None,
            max_damage: 0,
            fire_resistant: false,
            fuel_ticks: 0,
            food: None,
            tool: None,
            armor: None,
            wearable: None,
            hangs: None,
        }
    }

    /// A tool (`pickaxe`, `axe`, `shovel`, `hoe`, a mod's mineable tag
    /// `mymod:mineable/hammer`) or a `sword`, of `material` (a
    /// [`ToolMaterial`]: the game's `iron`… or a mod's), attacking as the
    /// game's tool of the kind does ([`Self::tool_attack`] changes it).
    pub fn tool(mut self, kind: impl Into<String>, material: impl Into<String>) -> Self {
        self.tool = Some(ToolItem {
            kind: kind.into(),
            material: material.into(),
            attack_damage: None,
            attack_speed: None,
        });
        self
    }

    /// The tool's own attack damage (on top of the material's) and speed
    /// change; none: the game's tool's. Nothing without [`Self::tool`].
    pub fn tool_attack(mut self, damage: Option<f32>, speed: Option<f32>) -> Self {
        if let Some(t) = &mut self.tool {
            t.attack_damage = damage;
            t.attack_speed = speed;
        }
        self
    }

    /// A piece of armor for `slot` (`head`, `chest`, `legs`, `feet`) of
    /// `material` (an [`ArmorMaterial`]: the game's `diamond`… or a mod's).
    pub fn armor(mut self, slot: ArmorSlot, material: impl Into<String>) -> Self {
        self.armor = Some(ArmorPiece {
            slot,
            material: material.into(),
        });
        self
    }

    /// Worn in `slot` without being armor.
    pub fn wearable(mut self, slot: ArmorSlot) -> Self {
        self.wearable = Some(slot);
        self
    }

    /// Used on a block's side, it hangs an `entity` (a hanging type) there,
    /// `size` blocks wide and high (`None`: the type's own); the entity
    /// keeps one of this item and drops it when it breaks.
    pub fn hangs(mut self, entity: impl Into<String>, size: Option<[u32; 2]>) -> Self {
        self.hangs = Some(Hangs {
            entity: entity.into(),
            size,
        });
        self
    }

    /// Maximum stack size (default 64). Ignored when `max_damage` is set.
    pub fn max_stack(mut self, n: u8) -> Self {
        self.max_stack = n;
        self
    }

    /// Display name shown in-game.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// A tooltip line shown on hover.
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Make this a damageable item (tool/weapon/armour). Forces stack size to 1.
    pub fn max_damage(mut self, durability: u32) -> Self {
        self.max_damage = durability;
        self
    }

    /// Make this item fire-resistant (won't burn in fire or lava).
    pub fn fire_resistant(mut self) -> Self {
        self.fire_resistant = true;
        self
    }

    /// Register as furnace fuel burning for `ticks` (200 ticks = 1 item smelted).
    pub fn fuel(mut self, ticks: u32) -> Self {
        self.fuel_ticks = ticks;
        self
    }

    /// Make this item edible with the given nutritional properties.
    pub fn food(mut self, food: FoodDef) -> Self {
        self.food = Some(food);
        self
    }
}
