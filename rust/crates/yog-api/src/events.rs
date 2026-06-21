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
