//! The server action table.

use super::*;

// ── Server action table (runtime → mod direction is wrong; it's mod → runtime) ─

/// All Minecraft-mutating calls available inside a handler.
///
/// `ctx` is an opaque pointer to the runtime's JNI state.  Every function takes
/// it as its first argument.  The pointer is valid for the lifetime of the process.
///
/// Strings **returned** by functions in this table are heap-allocated by the
/// runtime and must be freed with `free_str` after the caller has read them.
#[repr(C)]
pub struct YogServer {
    pub ctx: *mut c_void,
    pub abi_version: u32,
    /// `sizeof(YogServer)` at build time — allows mods compiled against an older
    /// table to detect and skip fields they don't know about.
    pub size: u32,

    /// Free a string returned by any function in this table.
    pub free_str: unsafe extern "C" fn(ptr: *mut u8, len: u32),

    // ── chat ─────────────────────────────────────────────────────────────────

    // ── world ────────────────────────────────────────────────────────────────

    // ── player ───────────────────────────────────────────────────────────────
    pub give_item:
        unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, item: YogStr, count: u32) -> bool,
    pub player_teleport:
        unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, pos: YogVec3) -> bool,
    pub send_to_player: unsafe extern "C" fn(
        ctx: *mut c_void,
        player: YogStr,
        channel: YogStr,
        data: *const u8,
        len: u32,
    ) -> bool,
    pub send_to_server:
        unsafe extern "C" fn(ctx: *mut c_void, channel: YogStr, data: *const u8, len: u32) -> bool,
    pub kick_player: unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, reason: YogStr) -> bool,
    pub set_gamemode: unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, mode: YogStr) -> bool,
    pub send_title: unsafe extern "C" fn(
        ctx: *mut c_void,
        player: YogStr,
        title: YogStr,
        sub: YogStr,
        fi: i32,
        stay: i32,
        fo: i32,
    ) -> bool,
    pub send_actionbar: unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, msg: YogStr) -> bool,
    pub play_sound: unsafe extern "C" fn(
        ctx: *mut c_void,
        dim: YogStr,
        pos: YogVec3,
        sound: YogStr,
        vol: f32,
        pitch: f32,
    ) -> bool,
    pub play_sound_player: unsafe extern "C" fn(
        ctx: *mut c_void,
        player: YogStr,
        sound: YogStr,
        vol: f32,
        pitch: f32,
    ) -> bool,

    // ── entity ───────────────────────────────────────────────────────────────
    pub entity_teleport: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, pos: YogVec3) -> bool,
    pub entity_position:
        unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, out: *mut YogVec3) -> bool,
    pub entity_health: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, out: *mut f32) -> bool,
    pub entity_set_health: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, hp: f32) -> bool,
    pub entity_kill: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr) -> bool,
    pub spawn_entity: unsafe extern "C" fn(
        ctx: *mut c_void,
        type_id: YogStr,
        dim: YogStr,
        pos: YogVec3,
    ) -> YogOwnedStr,
    pub entity_add_effect: unsafe extern "C" fn(
        ctx: *mut c_void,
        uuid: YogStr,
        fx: YogStr,
        dur: i32,
        amp: u8,
        particles: bool,
    ) -> bool,
    pub entity_remove_effect:
        unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, fx: YogStr) -> bool,
    pub entity_clear_effects: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr) -> bool,
    pub entity_velocity:
        unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, out: *mut YogVec3) -> bool,
    pub entity_set_velocity:
        unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, vel: YogVec3) -> bool,
    pub entity_add_velocity:
        unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, vel: YogVec3) -> bool,

    // ── tags & loot ──────────────────────────────────────────────────────────
    pub has_item_tag: unsafe extern "C" fn(ctx: *mut c_void, item: YogStr, tag: YogStr) -> bool,
    pub has_block_tag: unsafe extern "C" fn(ctx: *mut c_void, block: YogStr, tag: YogStr) -> bool,
    pub drop_loot:
        unsafe extern "C" fn(ctx: *mut c_void, table: YogStr, dim: YogStr, pos: YogVec3) -> bool,

    // ── scoreboard ───────────────────────────────────────────────────────────
    pub scoreboard_get:
        unsafe extern "C" fn(ctx: *mut c_void, obj: YogStr, player: YogStr, out: *mut i32) -> bool,
    pub scoreboard_set:
        unsafe extern "C" fn(ctx: *mut c_void, obj: YogStr, player: YogStr, score: i32) -> bool,
    pub scoreboard_add: unsafe extern "C" fn(
        ctx: *mut c_void,
        obj: YogStr,
        player: YogStr,
        delta: i32,
        out: *mut i32,
    ) -> bool,

    // ── boss bar ─────────────────────────────────────────────────────────────
    pub bossbar_create: unsafe extern "C" fn(
        ctx: *mut c_void,
        id: YogStr,
        title: YogStr,
        color: YogStr,
        style: YogStr,
    ) -> bool,
    pub bossbar_remove: unsafe extern "C" fn(ctx: *mut c_void, id: YogStr) -> bool,
    pub bossbar_set_title:
        unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, title: YogStr) -> bool,
    pub bossbar_set_progress:
        unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, progress: f32) -> bool,
    pub bossbar_set_color:
        unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, color: YogStr) -> bool,
    pub bossbar_add_player:
        unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, player: YogStr) -> bool,
    pub bossbar_remove_player:
        unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, player: YogStr) -> bool,
    pub bossbar_set_visible:
        unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, visible: bool) -> bool,

    // ── misc ─────────────────────────────────────────────────────────────────
    pub game_dir: unsafe extern "C" fn(ctx: *mut c_void) -> YogOwnedStr,

    // ── player query (ABI minor 4) ────────────────────────────────────────────
    /// Newline-separated list of online player names, or NONE if server not up.
    pub online_players: unsafe extern "C" fn(ctx: *mut c_void) -> YogOwnedStr,

    // ── block entity (NBT, ABI minor 3) ──────────────────────────────────────
    /// SNBT string of the block entity at `pos`, or NONE if there is none.
    pub get_block_nbt:
        unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> YogOwnedStr,
    /// Write SNBT into the block entity at `pos`. Returns false if no block entity exists.
    pub set_block_nbt:
        unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogBlockPos, snbt: YogStr) -> bool,

    // ── inventory (ABI minor 3) ───────────────────────────────────────────────
    /// Tab/newline-encoded inventory: one line per occupied slot, `slot\titem_id\tcount`.
    pub player_inventory: unsafe extern "C" fn(ctx: *mut c_void, player: YogStr) -> YogOwnedStr,
    /// Set (or clear when count==0) a specific inventory slot.
    pub player_set_slot: unsafe extern "C" fn(
        ctx: *mut c_void,
        player: YogStr,
        slot: u32,
        item_id: YogStr,
        count: u32,
    ) -> bool,

    // ── inventory-backed block slots (ABI minor 25, yog-inventory) ──────────
    /// `"item_id\tcount\tsnbt"` for the given slot of the block container
    /// at `pos`, or NONE if the slot is empty / there's no container there.
    pub get_inventory_slot: unsafe extern "C" fn(
        ctx: *mut c_void,
        dim: YogStr,
        pos: YogBlockPos,
        slot: u32,
    ) -> YogOwnedStr,
    /// Set (or clear when count==0) a slot of the block container at `pos`;
    /// `item_id` may carry the item's SNBT after a tab. Returns false if
    /// there's no container there.
    pub set_inventory_slot: unsafe extern "C" fn(
        ctx: *mut c_void,
        dim: YogStr,
        pos: YogBlockPos,
        slot: u32,
        item_id: YogStr,
        count: u32,
    ) -> bool,

    // ── cross-dimension teleport (ABI minor 3) ────────────────────────────────
    pub player_teleport_dim:
        unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, dim: YogStr, pos: YogVec3) -> bool,
    pub entity_teleport_dim:
        unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, dim: YogStr, pos: YogVec3) -> bool,

    // ── entity counting ───────────────────────────────────────────────────────
    /// Count loaded instances of `entity_type` in `dimension`. Returns -1 on error.
    pub world_entity_count:
        unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, entity_type: YogStr) -> i32,

    // ── entity NBT (ABI minor 6) ──────────────────────────────────────────────
    /// SNBT of the entity's persistent data, or NONE if entity not found.
    pub entity_get_nbt: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr,
    /// Merge SNBT data into the entity. Returns false if entity not found.
    pub entity_set_nbt: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, snbt: YogStr) -> bool,

    // ── particles (ABI minor 6) ───────────────────────────────────────────────
    /// Spawn `count` particles at `pos` in `dim`.
    /// `dx/dy/dz` control spread, `speed` controls particle speed.
    /// Returns false if the dimension or particle type is unknown.
    pub spawn_particles: unsafe extern "C" fn(
        ctx: *mut c_void,
        dim: YogStr,
        pos: YogVec3,
        particle_type: YogStr,
        count: i32,
        dx: f64,
        dy: f64,
        dz: f64,
        speed: f64,
    ) -> bool,

    // ── attributes (ABI minor 7) ──────────────────────────────────────────────
    /// Get the base value of an attribute on a living entity.
    /// `attribute_id` is a registry id, e.g. `"minecraft:generic.max_health"`.
    /// Returns `f64::NAN` if entity or attribute is not found.
    pub entity_attribute_get:
        unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, attribute_id: YogStr) -> f64,
    /// Set the base value of an attribute. Returns false if entity or attribute is not found.
    pub entity_attribute_set: unsafe extern "C" fn(
        ctx: *mut c_void,
        uuid: YogStr,
        attribute_id: YogStr,
        value: f64,
    ) -> bool,

    // ── held item NBT (ABI minor 11) ─────────────────────────────────────────
    /// SNBT of the item currently held in the player's main hand.
    /// Returns NONE if the player is offline or holding air.
    pub get_held_item_nbt: unsafe extern "C" fn(ctx: *mut c_void, player: YogStr) -> YogOwnedStr,
    /// Merge `snbt` data into the NBT of the player's held main-hand item in-place.
    /// Returns false if the player is offline or holding air.
    pub set_held_item_nbt:
        unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, snbt: YogStr) -> bool,

    // ── item stack query (ABI minor 12) ──────────────────────────────────────
    /// SNBT of the item in the player's off hand, or NONE if offline / holding air.
    pub get_offhand_item_nbt: unsafe extern "C" fn(ctx: *mut c_void, player: YogStr) -> YogOwnedStr,
    /// Merge `snbt` into the NBT of the player's off-hand item.
    /// Returns false if offline or holding air.
    pub set_offhand_item_nbt:
        unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, snbt: YogStr) -> bool,
    /// Full item stack at inventory `slot`: tab-separated `item_id\tcount\tsnbt`.
    /// `snbt` is `{}` when the item has no NBT. Returns NONE if offline or slot empty.
    pub get_slot_item:
        unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, slot: u32) -> YogOwnedStr,
    /// Replace inventory `slot` with an item stack. `snbt` may be empty to clear NBT.
    /// Pass `count == 0` to clear the slot (ignores `item_id` and `snbt`).
    pub set_slot_item: unsafe extern "C" fn(
        ctx: *mut c_void,
        player: YogStr,
        slot: u32,
        item_id: YogStr,
        count: u32,
        snbt: YogStr,
    ) -> bool,

    // ── entity rotation (ABI minor 25) ───────────────────────────────────────
    /// Yaw and pitch (degrees) of an entity by UUID. Returns false if the
    /// entity does not exist. `out` receives x=yaw, y=pitch, z=0.
    pub entity_rotation:
        unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, out: *mut YogVec3) -> bool,

    // ── world queries & effects (ABI minor 1) ────────────────────────────────
    // Records come back as one tab-separated line; `yog_core`'s `EntityInfo`,
    // `BlockHit`, `BlockInfo` and `PlayerInfo` document and decode them.
    /// `EntityInfo` line of a loaded entity, or NONE.
    pub entity_info: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr,
    /// Newline-separated uuids of the entities touching the box.
    pub entities_in_box: unsafe extern "C" fn(
        ctx: *mut c_void,
        dim: YogStr,
        min: YogVec3,
        max: YogVec3,
    ) -> YogOwnedStr,
    /// `BlockHit` line of the first block on the segment, or NONE.
    pub raycast_block: unsafe extern "C" fn(
        ctx: *mut c_void,
        dim: YogStr,
        from: YogVec3,
        to: YogVec3,
    ) -> YogOwnedStr,
    /// Uuid of the nearest entity on the segment (not `exclude`), or NONE.
    pub raycast_entity: unsafe extern "C" fn(
        ctx: *mut c_void,
        dim: YogStr,
        from: YogVec3,
        to: YogVec3,
        exclude: YogStr,
    ) -> YogOwnedStr,
    /// `BlockInfo` line of the block at `pos`, or NONE.
    pub block_info:
        unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> YogOwnedStr,
    pub break_block: unsafe extern "C" fn(
        ctx: *mut c_void,
        dim: YogStr,
        pos: YogBlockPos,
        drop: bool,
        breaker: YogStr,
    ) -> bool,
    pub place_block_from_slot: unsafe extern "C" fn(
        ctx: *mut c_void,
        player: YogStr,
        slot: u32,
        dim: YogStr,
        pos: YogBlockPos,
    ) -> bool,
    pub bonemeal: unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> bool,
    pub explode: unsafe extern "C" fn(
        ctx: *mut c_void,
        dim: YogStr,
        pos: YogVec3,
        power: f32,
        fire: bool,
        source: YogStr,
    ) -> bool,
    /// `json` is a JSON text component (the `/tellraw` format).
    pub tellraw: unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, json: YogStr) -> bool,
    pub entity_damage: unsafe extern "C" fn(
        ctx: *mut c_void,
        uuid: YogStr,
        amount: f32,
        damage_type: YogStr,
        attacker: YogStr,
    ) -> bool,
    pub entity_set_fire_ticks:
        unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, ticks: i32) -> bool,
    pub player_add_xp: unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, amount: i32) -> bool,
    pub player_set_may_fly:
        unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, may_fly: bool) -> bool,
    /// `PlayerInfo` line of an online player, or NONE.
    pub player_info: unsafe extern "C" fn(ctx: *mut c_void, player: YogStr) -> YogOwnedStr,
    /// Uuid of the dropped item entity, or NONE.
    pub spawn_item: unsafe extern "C" fn(
        ctx: *mut c_void,
        dim: YogStr,
        pos: YogVec3,
        vel: YogVec3,
        item_id: YogStr,
        count: u32,
        snbt: YogStr,
    ) -> YogOwnedStr,
    /// `item_id\tcount\tsnbt` of a dropped item entity, or NONE.
    pub item_entity_stack: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr,
    pub item_entity_set_stack: unsafe extern "C" fn(
        ctx: *mut c_void,
        uuid: YogStr,
        item_id: YogStr,
        count: u32,
        snbt: YogStr,
    ) -> bool,
    // ── ABI minor 2 ──
    /// `json` is a JSON text component.
    pub actionbar_json:
        unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, json: YogStr) -> bool,
    /// `face` is a `Direction` ordinal (down, up, north, south, west, east).
    pub use_item_on_block: unsafe extern "C" fn(
        ctx: *mut c_void,
        dim: YogStr,
        pos: YogBlockPos,
        face: u8,
        item_id: YogStr,
        player: YogStr,
    ) -> bool,
    pub pour_fluid: unsafe extern "C" fn(
        ctx: *mut c_void,
        dim: YogStr,
        pos: YogBlockPos,
        bucket_item_id: YogStr,
    ) -> bool,
    pub has_entity_tag:
        unsafe extern "C" fn(ctx: *mut c_void, entity_type: YogStr, tag_id: YogStr) -> bool,
    /// The block id a block item places, or NONE.
    pub item_block: unsafe extern "C" fn(ctx: *mut c_void, item_id: YogStr) -> YogOwnedStr,
    pub place_feature: unsafe extern "C" fn(
        ctx: *mut c_void,
        dim: YogStr,
        pos: YogBlockPos,
        feature_id: YogStr,
    ) -> bool,

    // ── ABI minor 3 ──────────────────────────────────────────────────────────
    pub set_item_cooldown:
        unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, item_id: YogStr, ticks: i32) -> bool,
    /// The UUID of what an entity rides, or NONE.
    pub entity_vehicle: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr,
    /// The UUIDs of what rides an entity directly, one a line (empty: none).
    pub entity_passengers: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr,
    /// Seats `rider` on `vehicle`; `force`: even one that wouldn't take it.
    pub entity_mount:
        unsafe extern "C" fn(ctx: *mut c_void, rider: YogStr, vehicle: YogStr, force: bool) -> bool,
    /// Gets an entity off what it rides.
    pub entity_dismount: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr) -> bool,
    /// Received redstone power 0–15, or -1 (no such dimension).
    pub redstone_power: unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> i32,
    /// Sets how far an entity has fallen (fall damage counts it on landing).
    pub entity_set_fall_distance: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, distance: f32) -> bool,
    /// The strongest signal the block gives off 0–15, or -1 (not a source).
    pub redstone_signal: unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> i32,
    pub set_redstone_output:
        unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogBlockPos, level: i32) -> bool,
    /// `"ns:id[prop=value,...]"`, or NONE.
    pub get_block_state: unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> YogOwnedStr,
    pub set_block_state:
        unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogBlockPos, state: YogStr) -> bool,
    pub world_is_thundering: unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr) -> bool,
    pub world_set_thunder:
        unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, thundering: bool, duration_ticks: i32) -> bool,
    /// Sets every block of the box `from..=to` to `state`; how many changed, or -1.
    pub fill_region: unsafe extern "C" fn(
        ctx: *mut c_void,
        dim: YogStr,
        from: YogBlockPos,
        to: YogBlockPos,
        state: YogStr,
    ) -> i32,
    /// The non-air blocks of the box `from..=to`: `"x y z state"` lines, or NONE.
    pub scan_region:
        unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, from: YogBlockPos, to: YogBlockPos) -> YogOwnedStr,

    // ── ABI 3.1 — entity dimensions ─────────────────────────────────────────
    /// An entity's own hitbox and eye height (blocks), or vanilla's again
    /// (`width` < 0); `false` if there's no such entity.
    pub entity_set_dimensions:
        unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, width: f32, height: f32, eye: f32) -> bool,

    // ── ABI 3.2 — entity data ───────────────────────────────────────────────
    /// Sets (`flags` & 1) or removes a value an entity keeps under `key`;
    /// `flags` & 2 saves it with the entity. The players who see the entity
    /// get it. `false` for an empty uuid or key.
    pub entity_data_set:
        unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, key: YogStr, data: *const u8, len: u32, flags: u32) -> bool,
    /// The value under `key`, written to `sink`; `false` if there's none.
    pub entity_data_get:
        unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, key: YogStr, sink: *mut c_void, write: YogBytesSink) -> bool,

    // ── ABI 3.5 — the world's save ──────────────────────────────────────────
    /// The directory of the world being played (its save: `saves/<world>`
    /// on a client, the level directory on a server) — where a mod keeps
    /// what belongs to this world. Free with `free_str`.
    pub world_dir: unsafe extern "C" fn(ctx: *mut c_void) -> YogOwnedStr,

    // ── ABI 3.6 — attribute values ──────────────────────────────────────────
    /// An attribute's value on a living entity — its base changed by every
    /// modifier (effects, equipment); `f64::NAN` if there's no such entity
    /// or attribute.
    pub entity_attribute_value: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, attribute_id: YogStr) -> f64,

    // ── ABI 3.7 — advancements ──────────────────────────────────────────────
    /// Awards advancement `id` to the player `uuid` (every criterion it has
    /// left); `false` if there's no such player or advancement.
    pub advancement_grant: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, id: YogStr) -> bool,
    /// Whether the player `uuid` has advancement `id`.
    pub advancement_done: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, id: YogStr) -> bool,
}

// ctx = *mut JavaVM which is global/stable. All fn ptrs are pure C-ABI.
unsafe impl Send for YogServer {}
unsafe impl Sync for YogServer {}
