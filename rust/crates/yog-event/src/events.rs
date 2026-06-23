//! Event types passed from Minecraft (through the Java host) into Rust mods.

use yog_core::BlockPos;

/// Whether a handler is running before or after the action.
///
/// In `Pre` phase the handler's return value may cancel the action.
/// In `Post` phase the return value is ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase { Pre, Post }

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

/// Fired when a player places a block (server side).
///
/// Passed to handlers registered via `Registry::on_player_place_block` with an
/// [`EventPhase`] argument:
/// - `Pre`  — fires before placement; return `false` to cancel.
/// - `Post` — fires after placement (requires mixin support; not yet wired).
#[derive(Debug, Clone)]
pub struct PlaceBlockEvent {
    pub player_name: String,
    /// Registry id of the block being placed, e.g. `minecraft:stone`.
    pub block_id: String,
    pub pos: BlockPos,
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

/// Fired when a player dies.
///
/// - `Pre`  — fires before death is processed; return `false` to prevent death
///            (entity survives at 0.5 HP).
/// - `Post` — fires after the player has died.
#[derive(Debug, Clone)]
pub struct PlayerDeathEvent {
    pub player_name: String,
    pub uuid: String,
    /// Damage source identifier, e.g. `"player"`, `"fall"`.
    pub source: String,
}

/// Fired when a player respawns after death (Post only).
#[derive(Debug, Clone)]
pub struct PlayerRespawnEvent {
    pub player_name: String,
    pub uuid: String,
    /// True if respawning at a bed or respawn anchor; false for world spawn.
    pub at_anchor: bool,
}

/// Fired when a player earns an advancement (Post only).
#[derive(Debug, Clone)]
pub struct AdvancementEvent {
    pub player_name: String,
    pub uuid: String,
    /// Namespaced id of the advancement, e.g. `"minecraft:story/mine_stone"`.
    pub advancement_id: String,
}

/// Fired when a player right-clicks (interacts with) an entity (server side).
///
/// - `Pre`  — fires before the interaction; return `false` to cancel.
/// - `Post` — fires after the interaction.
#[derive(Debug, Clone)]
pub struct EntityInteractEvent {
    pub player_name: String,
    pub player_uuid: String,
    /// Registry id of the interacted entity, e.g. `"minecraft:villager"`.
    pub entity_type: String,
    pub entity_uuid: String,
    /// `"main_hand"` or `"off_hand"`.
    pub hand: String,
}

/// Fired when a player takes a crafted item from a crafting output slot (Post only).
#[derive(Debug, Clone)]
pub struct CraftEvent {
    pub player_name: String,
    pub player_uuid: String,
    /// Registry id of the crafted item, e.g. `"minecraft:stick"`.
    pub result_item: String,
    pub result_count: u32,
}

/// Fired when an explosion occurs in a world.
///
/// - `Pre`  — fires before block destruction; return `false` to cancel
///            (blocks and entities are unaffected).
/// - `Post` — fires after the explosion has taken effect.
#[derive(Debug, Clone)]
pub struct ExplosionEvent {
    pub dimension: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub power: f32,
    /// UUID of the entity that caused the explosion, or empty string if none.
    pub cause_uuid: String,
}
