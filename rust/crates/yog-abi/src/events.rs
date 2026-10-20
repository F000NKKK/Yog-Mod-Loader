//! The event structs (Java → Rust).

use super::*;

// ── Event structs (Java → Rust) ───────────────────────────────────────────────

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

