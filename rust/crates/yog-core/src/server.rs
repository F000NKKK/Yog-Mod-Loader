//! The handle Rust mods use to act on the running server — the Rust → Minecraft
//! path.

use crate::BlockPos;

/// Low-level capabilities the Yog runtime exposes to mods (the engine contract).
///
/// The runtime provides the concrete implementation, backed by JNI calls into
/// the Java host; this crate stays JVM-free. Higher-level, ergonomic wrappers
/// live in the domain crates (e.g. `yog-world`'s `World`).
///
/// Dimensions are identified by their registry id string, e.g.
/// `"minecraft:overworld"`.
pub trait Server {
    /// Broadcast a chat message to all players on the server.
    fn broadcast(&self, message: &str);

    /// Registry id of the block at `pos` in `dimension`
    /// (e.g. `"minecraft:stone"`), or `None` if the dimension/position is
    /// unavailable. Call from the server thread (e.g. an event handler).
    fn get_block(&self, dimension: &str, pos: BlockPos) -> Option<String>;

    /// Set the block at `pos` in `dimension` to `block_id`. Returns whether the
    /// change was applied. Call from the server thread.
    fn set_block(&self, dimension: &str, pos: BlockPos, block_id: &str) -> bool;
}
