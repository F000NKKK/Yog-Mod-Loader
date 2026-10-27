//! What players do with items, blocks and entities.

use yog_data::Schema;

/// Fired every time a player sends a movement packet (very high frequency).
///
/// Post-phase only. The fields reflect the *new* position the client claims.
#[derive(Debug, Clone, Schema)]
pub struct Move {
    pub player_name: String,
    pub player_uuid: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub dimension: String,
}

/// Fired when a player right-clicks with an item (server side).
#[derive(Debug, Clone, Schema)]
pub struct UseItem {
    pub player_name: String,
    /// Registry id of the held item, e.g. `minecraft:stick`.
    pub item_id: String,
    /// Whether the player was sneaking (shift) during use.
    pub sneaking: bool,
    /// Whether it's the stack in the player's other hand.
    pub off_hand: bool,
    pub dimension: String,
}

/// Fired when a player attacks (left-clicks) an entity (server side).
#[derive(Debug, Clone, Schema)]
pub struct Attack {
    pub player_name: String,
    /// Registry id of the target, e.g. `minecraft:zombie`.
    pub target_type: String,
    /// Target entity UUID as a string.
    pub target_uuid: String,
    pub dimension: String,
}

/// Fired when a player right-clicks (interacts with) an entity (server side).
///
/// - `Pre`  — fires before the interaction; return `false` to cancel.
/// - `Post` — fires after the interaction.
#[derive(Debug, Clone, Schema)]
pub struct Interact {
    pub player_name: String,
    pub player_uuid: String,
    /// Registry id of the interacted entity, e.g. `"minecraft:villager"`.
    pub entity_type: String,
    pub entity_uuid: String,
    /// `"main_hand"` or `"off_hand"`.
    pub hand: String,
    pub dimension: String,
}

/// Fired when a player takes a crafted item from a crafting output slot (Post only).
#[derive(Debug, Clone, Schema)]
pub struct Craft {
    pub player_name: String,
    pub player_uuid: String,
    /// Registry id of the crafted item, e.g. `"minecraft:stick"`.
    pub result_item: String,
    pub result_count: u32,
    pub dimension: String,
}

/// Fired when a player picks up an item entity.
///
/// - `Pre`  — return `false` to prevent the pickup.
/// - `Post` — item was successfully picked up.
#[derive(Debug, Clone, Schema)]
pub struct Pickup {
    pub player_name: String,
    pub player_uuid: String,
    /// Registry id of the item, e.g. `"minecraft:diamond"`.
    pub item_id: String,
    pub item_count: u32,
    /// UUID of the item entity that was picked up.
    pub entity_uuid: String,
    pub dimension: String,
}
