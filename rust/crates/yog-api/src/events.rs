//! Event types passed from Minecraft (through the Java host) into Rust mods.

/// A block position in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

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
