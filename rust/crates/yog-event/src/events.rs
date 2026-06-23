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

// ── ABI minor 9 event types ───────────────────────────────────────────────────

/// Fired when a player picks up an item entity.
///
/// - `Pre`  — return `false` to prevent the pickup.
/// - `Post` — item was successfully picked up.
#[derive(Debug, Clone)]
pub struct ItemPickupEvent {
    pub player_name: String,
    pub player_uuid: String,
    /// Registry id of the item, e.g. `"minecraft:diamond"`.
    pub item_id: String,
    pub item_count: u32,
    /// UUID of the item entity that was picked up.
    pub entity_uuid: String,
}

/// Fired every time a player sends a movement packet (very high frequency).
///
/// Post-phase only. The fields reflect the *new* position the client claims.
#[derive(Debug, Clone)]
pub struct PlayerMoveEvent {
    pub player_name: String,
    pub player_uuid: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw:   f32,
    pub pitch: f32,
}

/// Fired when a player opens a container screen.
///
/// - `Pre`  — return `false` to prevent the screen from opening.
/// - `Post` — screen opened; `container_type` is set.
#[derive(Debug, Clone)]
pub struct ContainerOpenEvent {
    pub player_name: String,
    pub player_uuid: String,
    /// Screen handler registry id, e.g. `"minecraft:chest"`.
    /// Empty string for screens not in the registry (e.g. the player inventory).
    pub container_type: String,
}

/// Fired when a player closes a container screen (Post only).
#[derive(Debug, Clone)]
pub struct ContainerCloseEvent {
    pub player_name: String,
    pub player_uuid: String,
}

// ── ABI minor 10 — client-side events ────────────────────────────────────────

/// Fired every client tick on the render thread.
#[derive(Debug, Clone)]
pub struct ClientTickEvent {}

/// Fired every frame when the HUD is rendered.
/// `delta_tick` is the partial-tick interpolation factor (0.0–1.0).
#[derive(Debug, Clone)]
pub struct HudRenderEvent {
    pub delta_tick: f32,
}

/// Fired on every key press, release, or repeat (client-side).
///
/// Return `false` in the handler to prevent Minecraft from processing the key.
#[derive(Debug, Clone)]
pub struct KeyPressEvent {
    /// GLFW key code (e.g. 69 = E). See `org.lwjgl.glfw.GLFW`.
    pub key_code:  i32,
    pub scan_code: i32,
    /// 0 = release, 1 = press, 2 = repeat.
    pub action:    i32,
    /// Modifier bitmask: 1=Shift, 2=Ctrl, 4=Alt, 8=Super.
    pub modifiers: i32,
}

/// Fired when a GUI screen opens or closes.
#[derive(Debug, Clone)]
pub struct ScreenEvent {
    /// Simple class name of the screen, e.g. `"InventoryScreen"`, `"ChestScreen"`.
    pub screen_class: String,
}

/// Fired when a persistent projectile (arrow, trident, etc.) hits a target.
///
/// - `Pre`  — return `false` to cancel the hit (projectile passes through).
/// - `Post` — hit was processed.
#[derive(Debug, Clone)]
pub struct ProjectileHitEvent {
    /// Registry id of the projectile, e.g. `"minecraft:arrow"`.
    pub projectile_type: String,
    pub projectile_uuid: String,
    /// UUID of the entity that fired the projectile, or empty string.
    pub shooter_uuid: String,
    /// `"block"` or `"entity"`.
    pub hit_type: String,
    /// UUID of the entity that was hit (empty for block hits).
    pub hit_entity_uuid: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub dimension: String,
}
