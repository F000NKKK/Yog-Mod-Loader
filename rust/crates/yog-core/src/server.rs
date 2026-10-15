//! The handle Rust mods use to act on the running server — the Rust → Minecraft
//! path.

use crate::{BlockHit, BlockInfo, BlockPos, EntityInfo, Face, PlayerInfo};

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

    /// Set the block at `pos` in `dimension` to `block_id` — a registry id
    /// or, like `/setblock`, a state (`"minecraft:water_cauldron[level=3]"`).
    /// Returns whether the change was applied. Call from the server thread.
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
    fn play_sound_to_player(&self, player: &str, sound_id: &str, volume: f32, pitch: f32) -> bool;

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

    /// `(item_id, count, snbt)` of one slot of the block container at `pos`
    /// — a Yog inventory block (see `yog_inventory::InventoryDef`), a chest,
    /// any container. `snbt` is `"{}"` when the item carries no data.
    /// `None` if the slot is empty or there's no container there.
    fn get_inventory_slot(
        &self,
        dimension: &str,
        pos: BlockPos,
        slot: u32,
    ) -> Option<(String, u32, String)>;

    /// Puts `count` of `item_id`, carrying `snbt` (`""` for none), into one
    /// slot of the block container at `pos`; `count == 0` empties it.
    /// `false` if there's no container there (or the item id is unknown).
    fn set_inventory_slot(
        &self,
        dimension: &str,
        pos: BlockPos,
        slot: u32,
        item_id: &str,
        count: u32,
        snbt: &str,
    ) -> bool;

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
    /// `particle_type` is a particle id or, like `/particle`, an id with its
    /// options (`"minecraft:dust 1 0.5 0 1"`).
    /// `dx/dy/dz` control the spatial spread; `speed` controls particle velocity.
    fn spawn_particles(
        &self,
        dimension: &str,
        x: f64,
        y: f64,
        z: f64,
        particle_type: &str,
        count: i32,
        dx: f64,
        dy: f64,
        dz: f64,
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
    fn set_slot_item(&self, player: &str, slot: u32, item_id: &str, count: u32, snbt: &str)
    -> bool;

    // ── world queries & effects (ABI minor 1) ────────────────────────────────

    /// Type, position, size and kind of a loaded entity, or `None` if no
    /// entity has that uuid.
    fn entity_info(&self, uuid: &str) -> Option<EntityInfo>;

    /// Uuids of the entities whose bounding boxes touch the box `min`–`max`
    /// in `dimension`.
    fn entities_in_box(
        &self,
        dimension: &str,
        min: (f64, f64, f64),
        max: (f64, f64, f64),
    ) -> Vec<String>;

    /// The first block with a collision shape on the segment `from`–`to`
    /// (fluids are ignored).
    fn raycast_block(
        &self,
        dimension: &str,
        from: (f64, f64, f64),
        to: (f64, f64, f64),
    ) -> Option<BlockHit>;

    /// The nearest pickable entity whose (slightly inflated) bounding box the
    /// segment `from`–`to` crosses, other than `exclude` (a uuid, or `""`)
    /// and whatever it rides.
    fn raycast_entity(
        &self,
        dimension: &str,
        from: (f64, f64, f64),
        to: (f64, f64, f64),
        exclude: &str,
    ) -> Option<String>;

    /// State, hardness, fluid and redstone power of the block at `pos`.
    fn block_info(&self, dimension: &str, pos: BlockPos) -> Option<BlockInfo>;

    /// Break the block at `pos` as `breaker` (a player name, or `""`):
    /// plays the break effects and, if `drop`, drops its loot as if mined
    /// with an empty hand. Returns `false` for air and unbreakable blocks.
    fn break_block(&self, dimension: &str, pos: BlockPos, drop: bool, breaker: &str) -> bool;

    /// Place the block item in the player's inventory `slot` at `pos`, as if
    /// the player clicked the block below it: the item's own placement
    /// rules apply and one item is used up (unless creative).
    fn place_block_from_slot(
        &self,
        player: &str,
        slot: u32,
        dimension: &str,
        pos: BlockPos,
    ) -> bool;

    /// Apply bone meal to the block at `pos`. Returns whether it grew.
    fn bonemeal(&self, dimension: &str, pos: BlockPos) -> bool;

    /// An explosion of `power` (TNT is 4) at `pos`, caused by entity
    /// `source` (a uuid, or `""`); `fire` sets fires like a ghast fireball.
    #[allow(clippy::too_many_arguments)]
    fn explode(
        &self,
        dimension: &str,
        x: f64,
        y: f64,
        z: f64,
        power: f32,
        fire: bool,
        source: &str,
    ) -> bool;

    /// A system chat message to one player, like `/tellraw`: `json` is a
    /// JSON text component, so it can hold translation keys
    /// (`{"translate":"mymod.msg","with":["x"]}`) that each client shows in
    /// its own language.
    fn tellraw(&self, player: &str, json: &str) -> bool;

    /// Hurt an entity. `damage_type` is a damage type id
    /// (`"minecraft:magic"`, `"minecraft:drown"`, …; `""` picks one from
    /// the attacker: player attack, mob attack, or magic without one), and
    /// `attacker` the uuid of the entity responsible (or `""`). An unknown
    /// damage type falls back to magic. Returns whether it took the damage.
    fn entity_damage(&self, uuid: &str, amount: f32, damage_type: &str, attacker: &str) -> bool;

    /// Set an entity on fire for `ticks` (`0` puts it out).
    fn entity_set_fire_ticks(&self, uuid: &str, ticks: i32) -> bool;

    /// Give (or, negative, take) experience points.
    fn player_add_xp(&self, player: &str, amount: i32) -> bool;

    /// Let a player fly (creative-style) or take it away (also stops them
    /// flying). Game mode changes reset it.
    fn player_set_may_fly(&self, player: &str, may_fly: bool) -> bool;

    /// Game mode, selected slot, experience, air and flight of a player.
    fn player_info(&self, player: &str) -> Option<PlayerInfo>;

    /// Drop an item stack into the world with velocity `vel`; `snbt` is the
    /// stack's NBT (`""` for none). Returns the item entity's uuid.
    #[allow(clippy::too_many_arguments)]
    fn spawn_item(
        &self,
        dimension: &str,
        pos: (f64, f64, f64),
        vel: (f64, f64, f64),
        item_id: &str,
        count: u32,
        snbt: &str,
    ) -> Option<String>;

    /// The stack inside a dropped item entity: `(item_id, count, snbt)`.
    fn item_entity_stack(&self, uuid: &str) -> Option<(String, u32, String)>;

    /// Replace the stack inside a dropped item entity (`count == 0` removes
    /// the entity).
    fn item_entity_set_stack(&self, uuid: &str, item_id: &str, count: u32, snbt: &str) -> bool;

    /// An action-bar message (the line above the hotbar) from a JSON text
    /// component, like [`tellraw`](Self::tellraw): translation keys show
    /// in each client's own language.
    fn actionbar_json(&self, player: &str, json: &str) -> bool;

    /// Use one `item_id` (a fresh single item, nothing is taken from anyone)
    /// on the `face` of the block at `pos`, as `player` (a name, or `""` for
    /// nobody): flint and steel lights it, a shovel puts out a campfire, a
    /// hoe tills it… Returns whether the item did something.
    fn use_item_on_block(
        &self,
        dimension: &str,
        pos: BlockPos,
        face: Face,
        item_id: &str,
        player: &str,
    ) -> bool;

    /// Empty the fluid of a bucket item (`"minecraft:water_bucket"`) at `pos`
    /// the way a player would: it waterlogs a waterloggable block, replaces
    /// a replaceable one, and evaporates where water can't exist. Returns
    /// whether any fluid was placed.
    fn pour_fluid(&self, dimension: &str, pos: BlockPos, bucket_item_id: &str) -> bool;

    /// Whether an entity type (`"minecraft:zombie"`) is in `tag_id`
    /// (`"minecraft:skeletons"`).
    fn has_entity_tag(&self, entity_type: &str, tag_id: &str) -> bool;

    /// The block an item places (`"minecraft:torch"` for the torch item), or
    /// `None` if it isn't a block item.
    fn item_block(&self, item_id: &str) -> Option<String>;

    /// Place a configured feature (`"minecraft:oak"`, or a datapack's own
    /// `data/<ns>/worldgen/configured_feature/*.json`) at `pos`, the way
    /// a growing sapling places its tree. Returns whether it was placed.
    fn place_feature(&self, dimension: &str, pos: BlockPos, feature_id: &str) -> bool;

    // ── ABI minor 3 ──────────────────────────────────────────────────────────

    /// Puts `item_id` on cooldown for an online player for `ticks` (the
    /// grey sweep over it in the hotbar; using it waits). `false` if the
    /// player is offline or the item unknown.
    fn set_item_cooldown(&self, player: &str, item_id: &str, ticks: u32) -> bool;

    /// What an entity rides — a boat, a horse, a minecart — if anything.
    fn entity_vehicle(&self, uuid: &str) -> Option<String>;

    /// What rides an entity directly.
    fn entity_passengers(&self, uuid: &str) -> Vec<String>;

    /// Seats `rider` on `vehicle`; `force`: even on one that wouldn't take
    /// it (a full boat, a mob that doesn't carry riders).
    fn entity_mount(&self, rider: &str, vehicle: &str, force: bool) -> bool;

    /// Gets an entity off what it rides; `false` if it rode nothing.
    fn entity_dismount(&self, uuid: &str) -> bool;

    /// Gives the entity its own hitbox — `Some((width, height, eye
    /// height))` in blocks — or vanilla's again (`None`); it stays till
    /// changed, through pose changes. The server side: the clients are told
    /// with `yog_api::body::set_dimensions` (the mod's own packet). `false`
    /// if there's no such entity.
    fn entity_set_dimensions(&self, uuid: &str, size: Option<(f32, f32, f32)>) -> bool;

    /// Sets how far the entity has fallen — fall damage counts it when it
    /// lands (0: none). `false` if there's no such entity.
    fn entity_set_fall_distance(&self, uuid: &str, distance: f32) -> bool;

    /// The strongest signal the block at `pos` gives off (a lever, a torch,
    /// dust: its power); `None` if it isn't a redstone source.
    fn redstone_signal(&self, dimension: &str, pos: BlockPos) -> Option<u8>;

    /// The redstone power the block at `pos` receives from its neighbours
    /// (0–15); `None` if the dimension doesn't exist.
    fn redstone_power(&self, dimension: &str, pos: BlockPos) -> Option<u8>;

    /// Sets the power (0–15) the block at `pos` gives off — a block
    /// declared a redstone source (`BlockDef::redstone_source`,
    /// `"redstone": true` in its `.yblock`). Neighbours update.
    fn set_redstone_output(&self, dimension: &str, pos: BlockPos, level: u8) -> bool;

    /// The full state of the block at `pos`: its id and properties,
    /// `"minecraft:repeater[delay=2,facing=east,locked=false,powered=false]"`.
    fn get_block_state(&self, dimension: &str, pos: BlockPos) -> Option<String>;

    /// Places a block in a full state, as `/setblock` writes it
    /// (`"minecraft:repeater[delay=2,facing=east]"`; unset properties take
    /// their defaults). `false` if it doesn't parse.
    fn set_block_state(&self, dimension: &str, pos: BlockPos, state: &str) -> bool;

    /// Property `name` of the block at `pos` (`"energized"` → `"true"`).
    fn block_property(&self, dimension: &str, pos: BlockPos, name: &str) -> Option<String> {
        let state = self.get_block_state(dimension, pos)?;
        split_state(&state).1.into_iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string())
    }

    /// Sets some properties of the block at `pos` (`"energized=true,powered=false"`),
    /// keeping the rest. `false` if there's no block or the state doesn't take.
    fn set_block_properties(&self, dimension: &str, pos: BlockPos, properties: &str) -> bool {
        let Some(state) = self.get_block_state(dimension, pos) else { return false };
        let (id, mut props) = split_state(&state);
        for pair in properties.split(',').filter(|p| !p.is_empty()) {
            let Some((k, v)) = pair.split_once('=') else { return false };
            match props.iter_mut().find(|(n, _)| *n == k) {
                Some(p) => p.1 = v,
                None => props.push((k, v)),
            }
        }
        let props: Vec<String> = props.iter().map(|(k, v)| format!("{k}={v}")).collect();
        self.set_block_state(dimension, pos, &format!("{id}[{}]", props.join(",")))
    }

    /// Whether a thunderstorm is going on in `dimension`.
    fn world_is_thundering(&self, dimension: &str) -> bool;

    /// Starts (`true`) or ends a thunderstorm; `duration_ticks` 0 = a
    /// server-chosen length.
    fn world_set_thunder(&self, dimension: &str, thundering: bool, duration_ticks: i32) -> bool;

    /// Sets every block of the box between `from` and `to` (inclusive, any
    /// corners) to `state` (`"minecraft:barrier"`, `"minecraft:air"`, a full
    /// state) in one call. How many blocks changed; `None` if the dimension
    /// or state is unknown.
    fn fill_region(&self, dimension: &str, from: BlockPos, to: BlockPos, state: &str) -> Option<u32>;

    /// Every non-air block of the box between `from` and `to` with its full
    /// state (see [`Server::get_block_state`]) — one call for a whole area.
    fn scan_region(&self, dimension: &str, from: BlockPos, to: BlockPos) -> Vec<(BlockPos, String)>;
}

/// A block state (`"ns:id[k=v,...]"`) as its id and its properties.
pub fn split_state(state: &str) -> (&str, Vec<(&str, &str)>) {
    let (id, rest) = state.split_once('[').unwrap_or((state, ""));
    let props = rest
        .trim_end_matches(']')
        .split(',')
        .filter_map(|p| p.split_once('='))
        .collect();
    (id, props)
}

#[cfg(test)]
mod state_tests {
    use super::split_state;

    #[test]
    fn a_state_splits_into_its_id_and_properties() {
        assert_eq!(split_state("minecraft:stone"), ("minecraft:stone", vec![]));
        assert_eq!(
            split_state("hexcasting:impetus/look[energized=false,facing=up]"),
            ("hexcasting:impetus/look", vec![("energized", "false"), ("facing", "up")])
        );
    }
}
