//! The event structs (Java → Rust).

use super::*;

// ── Event structs (Java → Rust) ───────────────────────────────────────────────

#[repr(C)]
pub struct YogBlockBreakEvent {
    pub player: YogStr,
    pub block: YogStr,
    pub pos: YogBlockPos,
    /// Dimension the block was broken in, e.g. `minecraft:overworld`.
    pub dimension: YogStr,
}

#[repr(C)]
pub struct YogChatEvent {
    pub player: YogStr,
    pub message: YogStr,
    pub dimension: YogStr,
}

/// Shared by player_join and player_leave.
#[repr(C)]
pub struct YogPlayerEvent {
    pub player: YogStr,
    pub uuid: YogStr,
    pub dimension: YogStr,
}

#[repr(C)]
pub struct YogUseItemEvent {
    pub player: YogStr,
    pub item: YogStr,
    /// ABI minor 25: whether the player was sneaking (shift) during use.
    pub sneaking: bool,
    pub dimension: YogStr,
}

#[repr(C)]
pub struct YogUseBlockEvent {
    pub player: YogStr,
    pub block: YogStr,
    pub pos: YogBlockPos,
    pub dimension: YogStr,
    /// Whether the player was sneaking (shift).
    pub sneaking: bool,
}

#[repr(C)]
pub struct YogAttackEntityEvent {
    pub player: YogStr,
    pub target_type: YogStr,
    pub target_uuid: YogStr,
    pub dimension: YogStr,
}

#[repr(C)]
pub struct YogEntityDamageEvent {
    pub entity_type: YogStr,
    pub uuid: YogStr,
    pub amount: f32,
    pub source: YogStr,
    pub dimension: YogStr,
}

#[repr(C)]
pub struct YogEntityDeathEvent {
    pub entity_type: YogStr,
    pub uuid: YogStr,
    pub source: YogStr,
    pub dimension: YogStr,
}

#[repr(C)]
pub struct YogEntitySpawnEvent {
    pub entity_type: YogStr,
    pub uuid: YogStr,
    pub dimension: YogStr,
}

/// Fired when a player dies (Pre: before death is processed; Post: after death).
/// Pre phase — return false to prevent death (keep entity alive at 0.5 HP).
#[repr(C)]
pub struct YogPlayerDeathEvent {
    pub player: YogStr,
    pub uuid: YogStr,
    /// Damage source identifier, e.g. `"player"`, `"fall"`.
    pub source: YogStr,
    pub dimension: YogStr,
}

/// Fired when a player respawns after death.
#[repr(C)]
pub struct YogPlayerRespawnEvent {
    pub player: YogStr,
    pub uuid: YogStr,
    /// True if respawning at a bed or anchor, false at world spawn.
    pub at_anchor: bool,
    /// Dimension respawned into (may differ from where the player died).
    pub dimension: YogStr,
}

/// Fired when a player earns an advancement (Post only).
#[repr(C)]
pub struct YogAdvancementEvent {
    pub player: YogStr,
    pub uuid: YogStr,
    /// Namespaced id, e.g. `"minecraft:story/mine_stone"`.
    pub advancement: YogStr,
    pub dimension: YogStr,
}

/// Fired when a player right-clicks (interacts with) an entity (Pre: before; Post: after).
/// Pre phase — return false to cancel the interaction.
#[repr(C)]
pub struct YogEntityInteractEvent {
    pub player: YogStr,
    pub player_uuid: YogStr,
    pub entity_type: YogStr,
    pub entity_uuid: YogStr,
    /// `"main_hand"` or `"off_hand"`.
    pub hand: YogStr,
    pub dimension: YogStr,
}

/// Fired when a player takes a crafted item from a crafting output slot (Post only).
#[repr(C)]
pub struct YogCraftEvent {
    pub player: YogStr,
    pub player_uuid: YogStr,
    pub result_item: YogStr,
    pub result_count: u32,
    pub dimension: YogStr,
}

/// A machine (an inventory's `.yui` `<RecipeArea>`) and the recipe its
/// slots make. Pre: whether it may — the result shows only if every
/// handler allows it (a workbench short of a resource says no); `player`
/// empty. Post: a player took the result — the mod takes its share (the
/// host takes the ingredients).
#[repr(C)]
pub struct YogMachineCraftEvent {
    /// The machine's inventory, e.g. `mymod:press`.
    pub machine: YogStr,
    /// The recipe's kind and id.
    pub kind: YogStr,
    pub recipe: YogStr,
    pub result_item: YogStr,
    pub result_count: u32,
    /// The recipe's values (`{ "energy": 4 }`), as JSON.
    pub values: YogStr,
    pub pos: YogBlockPos,
    pub dimension: YogStr,
    pub player: YogStr,
}

/// Fired when an explosion occurs in a world (Pre: before block destruction; Post: after).
/// Pre phase — return false to cancel the explosion (block and entity damage suppressed).
#[repr(C)]
pub struct YogExplosionEvent {
    pub dimension: YogStr,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub power: f32,
    /// UUID of the entity that caused the explosion, or empty if world/tnt.
    pub cause_uuid: YogStr,
}

// ── ABI minor 9–10 event structs ──────────────────────────────────────────────

/// A hopper or dropper puts an item into a block that accepts items
/// (Pre: would it take it; Post: it takes it).
#[repr(C)]
pub struct YogItemInsertEvent {
    pub block: YogStr,
    pub pos: YogBlockPos,
    pub dimension: YogStr,
    pub item_id: YogStr,
    pub count: u32,
    /// The item's data as SNBT (`{}` for none).
    pub nbt: YogStr,
}

/// Fired when a player picks up an item entity (Pre: cancellable; Post: informational).
#[repr(C)]
pub struct YogItemPickupEvent {
    pub player: YogStr,
    pub player_uuid: YogStr,
    /// Registry id of the item, e.g. `"minecraft:diamond"`.
    pub item_id: YogStr,
    pub item_count: u32,
    /// UUID of the item entity that was picked up.
    pub entity_uuid: YogStr,
    pub dimension: YogStr,
}

/// Fired when a player sends a movement packet (Post only; very high frequency).
#[repr(C)]
pub struct YogPlayerMoveEvent {
    pub player: YogStr,
    pub player_uuid: YogStr,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub dimension: YogStr,
}

/// Fired when a player opens a container screen (Pre: cancellable; Post: informational).
/// `container_type` is the screen handler registry id, e.g. `"minecraft:chest"`.
/// Empty string if the type is not in the registry (e.g. the player inventory).
#[repr(C)]
pub struct YogContainerOpenEvent {
    pub player: YogStr,
    pub player_uuid: YogStr,
    pub container_type: YogStr,
    pub dimension: YogStr,
}

/// Fired when a player closes a container screen (Post only).
#[repr(C)]
pub struct YogContainerCloseEvent {
    pub player: YogStr,
    pub player_uuid: YogStr,
    pub dimension: YogStr,
}

/// Fired when a persistent projectile (arrow, trident) hits a target (Pre: cancellable).
/// Pre phase — return false to cancel the hit (projectile passes through).
#[repr(C)]
pub struct YogProjectileHitEvent {
    /// Registry id of the projectile, e.g. `"minecraft:arrow"`.
    pub projectile_type: YogStr,
    pub projectile_uuid: YogStr,
    /// UUID of the entity that shot/threw this projectile, or empty.
    pub shooter_uuid: YogStr,
    /// `"block"` or `"entity"`.
    pub hit_type: YogStr,
    /// UUID of the entity that was hit (empty for block hits).
    pub hit_entity_uuid: YogStr,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub dimension: YogStr,
}

/// Fired when a player places a block (Pre: before placement; Post: after).
#[repr(C)]
pub struct YogPlaceBlockEvent {
    pub player: YogStr,
    pub block: YogStr,
    pub pos: YogBlockPos,
    pub dimension: YogStr,
    /// Whether the player was sneaking (shift).
    pub sneaking: bool,
}

#[repr(C)]
pub struct YogPacketEvent {
    pub channel: YogStr,
    pub player: YogStr, // empty string on client-received packets
    pub payload: *const u8,
    pub payload_len: u32,
}

#[repr(C)]
pub struct YogCommandEvent {
    pub name: YogStr,
    pub args: YogStr,
    pub source: YogStr,
    pub uuid: YogStr,
    /// Empty for console-run commands (no world).
    pub dimension: YogStr,
}

// ── ABI minor 10 client event structs ─────────────────────────────────────────

/// Key press / release / repeat from the keyboard (client-side only).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogKeyPressEvent {
    /// GLFW key code (e.g. `GLFW_KEY_E = 69`).
    pub key_code: i32,
    pub scan_code: i32,
    /// 0 = release, 1 = press, 2 = repeat.
    pub action: i32,
    /// Modifier bitmask: 1=shift, 2=ctrl, 4=alt, 8=super.
    pub modifiers: i32,
}
