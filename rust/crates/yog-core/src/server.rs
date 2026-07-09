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

    /// Send a raw-byte packet to the server on `channel` (client → server).
    /// Only works in a client context; returns whether it was sent.
    fn send_to_server(&self, channel: &str, payload: &[u8]) -> bool;

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

    // ── status effects ──────────────────────────────────────────────────────

    /// Apply a status effect to a living entity. `effect_id` is a registry id
    /// such as `"minecraft:speed"` or `"minecraft:regeneration"`. `amplifier` is
    /// 0-based (0 = level I). Returns `false` if the entity or effect is unknown.
    fn entity_add_effect(
        &self,
        uuid: &str,
        effect_id: &str,
        duration_ticks: i32,
        amplifier: u8,
        show_particles: bool,
    ) -> bool;

    /// Remove a single status effect from a living entity.
    fn entity_remove_effect(&self, uuid: &str, effect_id: &str) -> bool;

    /// Clear all active status effects from a living entity.
    fn entity_clear_effects(&self, uuid: &str) -> bool;

    // ── loot tables ─────────────────────────────────────────────────────────

    /// Roll a loot table and spawn the resulting item entities in the world.
    /// `table_id` is the namespaced id, e.g. `"minecraft:entities/zombie"`.
    fn drop_loot(&self, table_id: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool;

    // ── tag queries ─────────────────────────────────────────────────────────

    /// Returns whether `item_id` (e.g. `"minecraft:stone"`) belongs to `tag_id`
    /// (e.g. `"minecraft:planks"`).
    fn has_item_tag(&self, item_id: &str, tag_id: &str) -> bool;

    /// Returns whether `block_id` belongs to `tag_id`.
    fn has_block_tag(&self, block_id: &str, tag_id: &str) -> bool;

    // ── world state ─────────────────────────────────────────────────────────

    /// Game time in ticks since world creation (never wraps, keeps counting).
    fn world_time(&self, dimension: &str) -> Option<i64>;

    /// Set the time-of-day (0 = dawn, 6000 = noon, 12000 = dusk, 18000 = midnight).
    /// Only changes the visual time, not the absolute world age.
    fn world_set_time(&self, dimension: &str, time: i64) -> bool;

    /// Whether it is currently raining in the given dimension.
    fn world_is_raining(&self, dimension: &str) -> bool;

    /// Start or stop rain. `duration_ticks` controls how long the weather lasts
    /// (use 0 for a server-chosen default duration).
    fn world_set_weather(&self, dimension: &str, raining: bool, duration_ticks: i32) -> bool;

    // ── entity velocity ─────────────────────────────────────────────────────

    /// Current velocity `(vx, vy, vz)` of any entity, or `None` if not loaded.
    fn entity_velocity(&self, uuid: &str) -> Option<(f64, f64, f64)>;

    /// Set the velocity of any entity. Returns `false` if not loaded.
    fn entity_set_velocity(&self, uuid: &str, vx: f64, vy: f64, vz: f64) -> bool;

    /// Add a velocity impulse (cumulative with existing velocity).
    fn entity_add_velocity(&self, uuid: &str, vx: f64, vy: f64, vz: f64) -> bool;

    // ── scoreboard ──────────────────────────────────────────────────────────

    /// Score of `player` on `objective`, or `None` if the objective doesn't exist.
    fn scoreboard_get(&self, objective: &str, player: &str) -> Option<i32>;

    /// Set the score; returns `false` if the objective doesn't exist.
    fn scoreboard_set(&self, objective: &str, player: &str, score: i32) -> bool;

    /// Add `delta` to the score (negative = subtract). Returns the new score,
    /// or `None` if the objective doesn't exist.
    fn scoreboard_add(&self, objective: &str, player: &str, delta: i32) -> Option<i32>;

    // ── sound ───────────────────────────────────────────────────────────────

    /// Play a sound at `(x, y, z)` in `dimension`. `sound_id` is a registry id
    /// (e.g. `"minecraft:entity.player.levelup"`). All players within range hear
    /// it. Returns `false` if the dimension is unknown.
    fn play_sound(
        &self,
        dimension: &str,
        x: f64,
        y: f64,
        z: f64,
        sound_id: &str,
        volume: f32,
        pitch: f32,
    ) -> bool;

    /// Play a sound at the named player's current position. All players nearby
    /// (including the target) hear it. Returns `false` if the player is offline.
    fn play_sound_to_player(
        &self,
        player: &str,
        sound_id: &str,
        volume: f32,
        pitch: f32,
    ) -> bool;

    // ── title / actionbar ───────────────────────────────────────────────────

    /// Send a title+subtitle screen to a player. Pass empty strings to omit
    /// either line. Timings are in ticks (20 ticks = 1 second).
    fn send_title(
        &self,
        player: &str,
        title: &str,
        subtitle: &str,
        fadein: i32,
        stay: i32,
        fadeout: i32,
    ) -> bool;

    /// Send a short message to the action-bar (the line just above the hotbar).
    fn send_actionbar(&self, player: &str, message: &str) -> bool;

    /// Yaw and pitch (degrees) of an entity by UUID, or `None` if it does not
    /// exist. Minecraft convention: yaw 0 = south, positive pitch looks down.
    fn entity_rotation(&self, uuid: &str) -> Option<(f32, f32)>;

    // ── player management ───────────────────────────────────────────────────

    /// Disconnect `player` with the given `reason` message.
    fn kick_player(&self, player: &str, reason: &str) -> bool;

    /// Change a player's game mode. `gamemode` is one of `"survival"`,
    /// `"creative"`, `"adventure"`, `"spectator"` (or the abbreviations
    /// `"s"`, `"c"`, `"a"`, `"sp"`). Returns `false` if the player is offline
    /// or `gamemode` is unrecognised.
    fn set_gamemode(&self, player: &str, gamemode: &str) -> bool;

    // ── boss bar ────────────────────────────────────────────────────────────

    /// Create a new boss bar identified by `id` (a namespaced id such as
    /// `"mymod:progress"`). `color`: `"pink"` / `"blue"` / `"red"` / `"green"` /
    /// `"yellow"` / `"purple"` / `"white"`. `style`: `"progress"` /
    /// `"notched_6"` / `"notched_10"` / `"notched_12"` / `"notched_20"`.
    /// Returns `false` if a bar with that id already exists.
    fn bossbar_create(&self, id: &str, title: &str, color: &str, style: &str) -> bool;

    /// Remove a boss bar (also removes it from all players). Returns `false` if
    /// the bar doesn't exist.
    fn bossbar_remove(&self, id: &str) -> bool;

    /// Update the displayed title of a boss bar.
    fn bossbar_set_title(&self, id: &str, title: &str) -> bool;

    /// Set the fill level of a boss bar (0.0 = empty, 1.0 = full).
    fn bossbar_set_progress(&self, id: &str, progress: f32) -> bool;

    /// Change the color of a boss bar (same color names as [`bossbar_create`]).
    fn bossbar_set_color(&self, id: &str, color: &str) -> bool;

    /// Add an online player to the boss bar's audience.
    fn bossbar_add_player(&self, id: &str, player: &str) -> bool;

    /// Remove a player from the boss bar's audience.
    fn bossbar_remove_player(&self, id: &str, player: &str) -> bool;

    /// Show or hide a boss bar for all its current audience members.
    fn bossbar_set_visible(&self, id: &str, visible: bool) -> bool;

    // ── misc ────────────────────────────────────────────────────────────────

    /// Absolute path of the game / server root directory.
    fn game_dir(&self) -> String;

    /// Names of all currently connected players.
    fn online_players(&self) -> Vec<String>;

    // ── block entity ─────────────────────────────────────────────────────────

    /// SNBT string of the block entity at `pos` (e.g. chest contents, furnace
    /// state, sign text). Returns `None` if there is no block entity there.
    fn get_block_nbt(&self, dimension: &str, pos: BlockPos) -> Option<String>;

    /// Write `snbt` data into the block entity at `pos` and mark it dirty.
    /// Returns `false` if there is no block entity at that position.
    fn set_block_nbt(&self, dimension: &str, pos: BlockPos, snbt: &str) -> bool;

    // ── inventory-backed block slots (yog-inventory) ─────────────────────────

    /// `(item_id, count)` of one slot of the inventory-backed block at `pos`
    /// (see `yog_inventory::InventoryDef`). Returns `None` if the slot is
    /// empty, or there is no such inventory at that position.
    fn get_inventory_slot(&self, dimension: &str, pos: BlockPos, slot: u32) -> Option<(String, u32)>;

    /// Set (or clear when `count == 0`) one slot of the inventory-backed
    /// block at `pos`. Returns `false` if there is no such inventory there.
    fn set_inventory_slot(&self, dimension: &str, pos: BlockPos, slot: u32, item_id: &str, count: u32) -> bool;

    // ── inventory ────────────────────────────────────────────────────────────

    /// All occupied inventory slots of an online player.
    /// Returns one entry per occupied slot: `(slot_index, item_id, count)`.
    fn player_inventory(&self, player: &str) -> Vec<(u32, String, u32)>;

    /// Set (or clear when `count == 0`) one inventory slot of an online player.
    fn player_set_slot(&self, player: &str, slot: u32, item_id: &str, count: u32) -> bool;

    // ── cross-dimension teleport ─────────────────────────────────────────────

    /// Teleport a player to `(x, y, z)` in a different (or same) dimension.
    fn teleport_to_dim(&self, player: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool;

    /// Teleport any entity (by UUID) to `(x, y, z)` in `dimension`.
    fn entity_teleport_to_dim(&self, uuid: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool;

    // ── entity query ─────────────────────────────────────────────────────────

    /// Number of loaded entities of `entity_type` (e.g. `"minecraft:zombie"`)
    /// in `dimension`. Returns `-1` if the dimension or entity type is unknown.
    fn world_entity_count(&self, dimension: &str, entity_type: &str) -> i32;

    // ── entity NBT ───────────────────────────────────────────────────────────

    /// SNBT string of the entity's persistent NBT, or `None` if not found.
    fn entity_get_nbt(&self, uuid: &str) -> Option<String>;

    /// Merge SNBT data into the entity's persistent NBT. Returns `false` if not found.
    fn entity_set_nbt(&self, uuid: &str, snbt: &str) -> bool;

    // ── particles ────────────────────────────────────────────────────────────

    /// Spawn `count` particles at `(x, y, z)` in `dimension`.
    /// `dx/dy/dz` control the spatial spread; `speed` controls particle velocity.
    fn spawn_particles(
        &self,
        dimension: &str,
        x: f64, y: f64, z: f64,
        particle_type: &str,
        count: i32,
        dx: f64, dy: f64, dz: f64,
        speed: f64,
    ) -> bool;

    // ── attributes ───────────────────────────────────────────────────────────

    /// Get the base value of an attribute on a living entity.
    /// `attribute_id` e.g. `"minecraft:generic.max_health"`.
    /// Returns `None` if the entity or attribute is not found.
    fn entity_attribute_get(&self, uuid: &str, attribute_id: &str) -> Option<f64>;

    /// Set the base value of an attribute. Returns false if not found.
    fn entity_attribute_set(&self, uuid: &str, attribute_id: &str, value: f64) -> bool;

    // ── held item NBT (ABI minor 11) ─────────────────────────────────────────

    /// SNBT string of the item currently held in the player's main hand.
    /// Returns `None` if the player is offline or holding air.
    fn get_held_item_nbt(&self, player: &str) -> Option<String>;

    /// Merge `snbt` data into the NBT of the item in the player's main hand.
    /// Returns `false` if the player is offline or holding air.
    fn set_held_item_nbt(&self, player: &str, snbt: &str) -> bool;

    // ── item stack query (ABI minor 12) ──────────────────────────────────────

    /// SNBT of the item in the player's off hand.
    /// Returns `None` if offline or holding air.
    fn get_offhand_item_nbt(&self, player: &str) -> Option<String>;

    /// Merge `snbt` into the NBT of the player's off-hand item.
    /// Returns `false` if offline or holding air.
    fn set_offhand_item_nbt(&self, player: &str, snbt: &str) -> bool;

    /// Full item stack at inventory `slot`: `(item_id, count, nbt_snbt)`.
    /// `nbt_snbt` is `"{}"` when the item has no NBT.
    /// Returns `None` if the player is offline or the slot is empty.
    fn get_slot_item(&self, player: &str, slot: u32) -> Option<(String, u32, String)>;

    /// Replace inventory `slot`. Pass `count == 0` to clear the slot.
    /// `snbt` is merged into the new item's NBT (pass `""` for no NBT).
    fn set_slot_item(&self, player: &str, slot: u32, item_id: &str, count: u32, snbt: &str) -> bool;
}
