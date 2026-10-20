//! The event structs (Java → Rust).

use super::*;

// ── Event structs (Java → Rust) ───────────────────────────────────────────────

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
