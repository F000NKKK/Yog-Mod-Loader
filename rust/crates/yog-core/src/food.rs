//! What eating an item gives, as the game says (`yog_event`'s
//! `item::Food`): the same on every version, each host reading its own
//! game's food data — mods' food included.

use yog_data::{Atom, Schema};

/// An item's food: what it fills, how long it holds off hunger, what it does.
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct Food {
    /// Hunger points it fills (a drumstick is 2).
    pub nutrition: i32,
    /// Saturation it gives, in points (not the older versions' modifier).
    pub saturation: f32,
    /// Eaten even when full (a golden apple).
    pub always_edible: bool,
    /// Eaten faster than most (dried kelp).
    pub fast: bool,
    /// The effects it may give.
    pub effects: Vec<FoodEffect>,
}

/// An effect eating may give.
#[derive(Debug, Clone, PartialEq, Schema)]
pub struct FoodEffect {
    /// The effect's id (`minecraft:poison`).
    pub effect: Atom,
    /// How likely, 0 to 1.
    pub chance: f32,
    /// Whether the game counts it good for who has it (regeneration) or not (poison).
    pub beneficial: bool,
}
