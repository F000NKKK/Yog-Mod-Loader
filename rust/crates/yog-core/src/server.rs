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

    /// Give `count` of `item_id` to the named player. Returns whether it worked
    /// (player online, item valid). Call from the server thread.
    fn give_item(&self, player: &str, item_id: &str, count: u32) -> bool;

    /// Teleport the named player to `(x, y, z)` in their current world. Returns
    /// whether it worked. Call from the server thread.
    fn teleport(&self, player: &str, x: f64, y: f64, z: f64) -> bool;

    /// Send a raw-byte packet to the named player on `channel` (server → client).
    /// Returns whether the player was online. Payload is opaque bytes — no NBT.
    fn send_to_player(&self, player: &str, channel: &str, payload: &[u8]) -> bool;

    // ── entity (universal, by UUID) ─────────────────────────────────────────

    /// Teleport any entity (by UUID) within its current world.
    fn entity_teleport(&self, uuid: &str, x: f64, y: f64, z: f64) -> bool;

    /// Position of an entity, or `None` if not loaded.
    fn entity_position(&self, uuid: &str) -> Option<(f64, f64, f64)>;

    /// Health of a living entity, or `None`.
    fn entity_health(&self, uuid: &str) -> Option<f32>;

    /// Set a living entity's health; returns whether it applied.
    fn entity_set_health(&self, uuid: &str, health: f32) -> bool;

    /// Remove/kill an entity.
    fn entity_kill(&self, uuid: &str) -> bool;

    /// Spawn an entity of `entity_type` (e.g. `minecraft:pig`) at a position;
    /// returns its UUID, or `None` on failure.
    fn spawn_entity(
        &self,
        entity_type: &str,
        dimension: &str,
        x: f64,
        y: f64,
        z: f64,
    ) -> Option<String>;
}
