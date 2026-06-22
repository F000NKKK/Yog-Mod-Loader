//! Event types passed from Minecraft (through the Java host) into Rust mods.

use yog_core::BlockPos;

/// Fired when a player breaks a block (server side).
#[derive(Debug, Clone)]
pub struct BlockBreakEvent {
    pub player_name: String,
    /// Registry id of the block, e.g. `minecraft:stone`.
    pub block_id: String,
    pub pos: BlockPos,
}

/// Fired when a player sends a chat message.
#[derive(Debug, Clone)]
pub struct ChatEvent {
    pub player_name: String,
    pub message: String,
}

/// Fired when a player joins the server.
#[derive(Debug, Clone)]
pub struct PlayerJoinEvent {
    pub player_name: String,
    /// Player UUID as a string, e.g. `069a79f4-44e9-4726-a5be-fca90e38aaf5`.
    pub uuid: String,
}

/// Fired when a player leaves the server.
#[derive(Debug, Clone)]
pub struct PlayerLeaveEvent {
    pub player_name: String,
    pub uuid: String,
}

/// Fired when a player right-clicks with an item (server side).
#[derive(Debug, Clone)]
pub struct UseItemEvent {
    pub player_name: String,
    /// Registry id of the held item, e.g. `minecraft:stick`.
    pub item_id: String,
}

/// Fired when a player right-clicks a block (server side).
#[derive(Debug, Clone)]
pub struct UseBlockEvent {
    pub player_name: String,
    /// Registry id of the targeted block, e.g. `minecraft:chest`.
    pub block_id: String,
    pub pos: BlockPos,
}

/// Fired when a player attacks (left-clicks) an entity (server side).
#[derive(Debug, Clone)]
pub struct AttackEntityEvent {
    pub player_name: String,
    /// Registry id of the target, e.g. `minecraft:zombie`.
    pub target_type: String,
    /// Target entity UUID as a string.
    pub target_uuid: String,
}

/// Fired after a living entity takes damage (server side).
#[derive(Debug, Clone)]
pub struct EntityDamageEvent {
    /// Registry id of the entity, e.g. `minecraft:zombie`.
    pub entity_type: String,
    pub uuid: String,
    /// Amount of damage dealt (hit points).
    pub amount: f32,
    /// Identifier of the damage source, e.g. `minecraft:player`, `fall`.
    pub source: String,
}

/// Fired after a living entity dies (server side).
#[derive(Debug, Clone)]
pub struct EntityDeathEvent {
    /// Registry id of the entity, e.g. `minecraft:zombie`.
    pub entity_type: String,
    pub uuid: String,
    /// Identifier of the killing damage source, e.g. `minecraft:player`.
    pub source: String,
}

/// Fired when any entity is loaded into a world (server side).
///
/// Also used for `on_entity_spawn_pre` (cancellable): return `false` to
/// discard the entity immediately after loading (effective spawn cancellation).
#[derive(Debug, Clone)]
pub struct EntitySpawnEvent {
    /// Registry id of the entity, e.g. `minecraft:zombie`.
    pub entity_type: String,
    pub uuid: String,
    /// Dimension the entity was added to, e.g. `minecraft:overworld`.
    pub dimension: String,
}
