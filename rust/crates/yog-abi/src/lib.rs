//! Yog stable C ABI — the ONLY types that cross the mod/runtime boundary.
//!
//! Rules this file must never break:
//!  - Every type is `#[repr(C)]`.
//!  - No Rust trait objects, no generics, no std types in public structs.
//!  - New fields are appended only at the END of structs; increment `ABI_MINOR`.
//!  - ABI_MAJOR bumps only when an existing field is removed or reordered.
//!
//! Mods and the runtime are compiled independently. They are compatible when
//! `ABI_MAJOR` matches and `mod_ABI_MINOR <= runtime_ABI_MINOR`.

use std::os::raw::c_void;

// ── Version ──────────────────────────────────────────────────────────────────

pub const ABI_MAJOR: u32 = 0;
pub const ABI_MINOR: u32 = 10;
/// `ABI_MAJOR * 10_000 + ABI_MINOR`.  Checked at mod load time.
pub const ABI_VERSION: u32 = ABI_MAJOR * 10_000 + ABI_MINOR;

// ── Primitive types ───────────────────────────────────────────────────────────

/// Borrowed UTF-8 byte slice passed to a function. NOT null-terminated.
/// Valid only for the duration of the call that provides it — never store.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogStr {
    pub ptr: *const u8,
    pub len: u32,
}

impl YogStr {
    pub const EMPTY: Self = Self { ptr: std::ptr::null(), len: 0 };

    #[inline]
    pub fn from_str(s: &str) -> Self {
        Self { ptr: s.as_ptr(), len: s.len() as u32 }
    }

    #[inline]
    pub fn is_empty(self) -> bool { self.len == 0 || self.ptr.is_null() }

    /// SAFETY: `ptr` must be valid UTF-8 of `len` bytes for at least the
    /// duration of the call that provided this `YogStr`.
    #[inline]
    pub unsafe fn as_str<'a>(self) -> &'a str {
        if self.ptr.is_null() || self.len == 0 {
            return "";
        }
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(self.ptr, self.len as usize))
    }
}

/// Heap-allocated UTF-8 string owned by the RUNTIME.
/// `ptr == null` encodes `None` / "not found".
/// When `ptr` is non-null the caller MUST free it via `YogServer::free_str`.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogOwnedStr {
    pub ptr: *mut u8,
    pub len: u32,
}

impl YogOwnedStr {
    pub const NONE: Self = Self { ptr: std::ptr::null_mut(), len: 0 };

    /// Allocate a new `YogOwnedStr` from a Rust `String`.
    pub fn from_string(s: String) -> Self {
        let len = s.len() as u32;
        let ptr = Box::into_raw(s.into_bytes().into_boxed_slice()) as *mut u8;
        Self { ptr, len }
    }

    #[inline]
    pub fn is_none(self) -> bool { self.ptr.is_null() }
}

/// Integer 3-D block position.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogBlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Float 3-D vector (position, velocity, …).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

// ── Event structs (Java → Rust) ───────────────────────────────────────────────

#[repr(C)]
pub struct YogBlockBreakEvent {
    pub player: YogStr,
    pub block:  YogStr,
    pub pos:    YogBlockPos,
}

#[repr(C)]
pub struct YogChatEvent {
    pub player:  YogStr,
    pub message: YogStr,
}

/// Shared by player_join and player_leave.
#[repr(C)]
pub struct YogPlayerEvent {
    pub player: YogStr,
    pub uuid:   YogStr,
}

#[repr(C)]
pub struct YogUseItemEvent {
    pub player: YogStr,
    pub item:   YogStr,
}

#[repr(C)]
pub struct YogUseBlockEvent {
    pub player: YogStr,
    pub block:  YogStr,
    pub pos:    YogBlockPos,
}

#[repr(C)]
pub struct YogAttackEntityEvent {
    pub player:      YogStr,
    pub target_type: YogStr,
    pub target_uuid: YogStr,
}

#[repr(C)]
pub struct YogEntityDamageEvent {
    pub entity_type: YogStr,
    pub uuid:        YogStr,
    pub amount:      f32,
    pub source:      YogStr,
}

#[repr(C)]
pub struct YogEntityDeathEvent {
    pub entity_type: YogStr,
    pub uuid:        YogStr,
    pub source:      YogStr,
}

#[repr(C)]
pub struct YogEntitySpawnEvent {
    pub entity_type: YogStr,
    pub uuid:        YogStr,
    pub dimension:   YogStr,
}

/// Fired when a player dies (Pre: before death is processed; Post: after death).
/// Pre phase — return false to prevent death (keep entity alive at 0.5 HP).
#[repr(C)]
pub struct YogPlayerDeathEvent {
    pub player: YogStr,
    pub uuid:   YogStr,
    /// Damage source identifier, e.g. `"player"`, `"fall"`.
    pub source: YogStr,
}

/// Fired when a player respawns after death.
#[repr(C)]
pub struct YogPlayerRespawnEvent {
    pub player:    YogStr,
    pub uuid:      YogStr,
    /// True if respawning at a bed or anchor, false at world spawn.
    pub at_anchor: bool,
}

/// Fired when a player earns an advancement (Post only).
#[repr(C)]
pub struct YogAdvancementEvent {
    pub player:      YogStr,
    pub uuid:        YogStr,
    /// Namespaced id, e.g. `"minecraft:story/mine_stone"`.
    pub advancement: YogStr,
}

/// Fired when a player right-clicks (interacts with) an entity (Pre: before; Post: after).
/// Pre phase — return false to cancel the interaction.
#[repr(C)]
pub struct YogEntityInteractEvent {
    pub player:      YogStr,
    pub player_uuid: YogStr,
    pub entity_type: YogStr,
    pub entity_uuid: YogStr,
    /// `"main_hand"` or `"off_hand"`.
    pub hand:        YogStr,
}

/// Fired when a player takes a crafted item from a crafting output slot (Post only).
#[repr(C)]
pub struct YogCraftEvent {
    pub player:       YogStr,
    pub player_uuid:  YogStr,
    pub result_item:  YogStr,
    pub result_count: u32,
}

/// Fired when an explosion occurs in a world (Pre: before block destruction; Post: after).
/// Pre phase — return false to cancel the explosion (block and entity damage suppressed).
#[repr(C)]
pub struct YogExplosionEvent {
    pub dimension:    YogStr,
    pub x:            f64,
    pub y:            f64,
    pub z:            f64,
    pub power:        f32,
    /// UUID of the entity that caused the explosion, or empty if world/tnt.
    pub cause_uuid:   YogStr,
}

// ── ABI minor 9–10 event structs ──────────────────────────────────────────────

/// Fired when a player picks up an item entity (Pre: cancellable; Post: informational).
#[repr(C)]
pub struct YogItemPickupEvent {
    pub player:      YogStr,
    pub player_uuid: YogStr,
    /// Registry id of the item, e.g. `"minecraft:diamond"`.
    pub item_id:     YogStr,
    pub item_count:  u32,
    /// UUID of the item entity that was picked up.
    pub entity_uuid: YogStr,
}

/// Fired when a player sends a movement packet (Post only; very high frequency).
#[repr(C)]
pub struct YogPlayerMoveEvent {
    pub player:      YogStr,
    pub player_uuid: YogStr,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw:   f32,
    pub pitch: f32,
}

/// Fired when a player opens a container screen (Pre: cancellable; Post: informational).
/// `container_type` is the screen handler registry id, e.g. `"minecraft:chest"`.
/// Empty string if the type is not in the registry (e.g. the player inventory).
#[repr(C)]
pub struct YogContainerOpenEvent {
    pub player:         YogStr,
    pub player_uuid:    YogStr,
    pub container_type: YogStr,
}

/// Fired when a player closes a container screen (Post only).
#[repr(C)]
pub struct YogContainerCloseEvent {
    pub player:      YogStr,
    pub player_uuid: YogStr,
}

/// Fired when a persistent projectile (arrow, trident) hits a target (Pre: cancellable).
/// Pre phase — return false to cancel the hit (projectile passes through).
#[repr(C)]
pub struct YogProjectileHitEvent {
    /// Registry id of the projectile, e.g. `"minecraft:arrow"`.
    pub projectile_type: YogStr,
    pub projectile_uuid: YogStr,
    /// UUID of the entity that shot/threw this projectile, or empty.
    pub shooter_uuid:    YogStr,
    /// `"block"` or `"entity"`.
    pub hit_type:        YogStr,
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
    pub block:  YogStr,
    pub pos:    YogBlockPos,
}

#[repr(C)]
pub struct YogPacketEvent {
    pub channel:     YogStr,
    pub player:      YogStr, // empty string on client-received packets
    pub payload:     *const u8,
    pub payload_len: u32,
}

#[repr(C)]
pub struct YogCommandEvent {
    pub name:   YogStr,
    pub args:   YogStr,
    pub source: YogStr,
    pub uuid:   YogStr,
}

// ── Content definition structs (mod → runtime) ────────────────────────────────

#[repr(C)]
pub struct YogItemDef {
    pub id:              YogStr,
    pub max_stack:       u32,
    pub name:            YogStr, // empty = no override
    pub tooltip:         YogStr, // empty = none
    pub max_damage:      u32,
    pub fire_resistant:  bool,
    pub fuel_ticks:      u32,
    pub food_nutrition:  u32,    // 0 = not a food item
    pub food_saturation: f32,
    pub food_always_eat: bool,
}

#[repr(C)]
pub struct YogBlockDef {
    pub id:            YogStr,
    pub hardness:      f32,
    pub resistance:    f32,
    pub name:          YogStr,
    pub light_level:   u8,
    pub sound:         YogStr,   // empty = default stone sound
    pub requires_tool: bool,
    pub no_collision:  bool,
    pub slipperiness:  f32,
    /// Bounding box in pixels: `[x1, y1, z1, x2, y2, z2]`. All zeros = full cube.
    pub shape:         [f32; 6],
}

// ── ABI minor 10 client event structs ─────────────────────────────────────────

/// Key press / release / repeat from the keyboard (client-side only).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct YogKeyPressEvent {
    /// GLFW key code (e.g. `GLFW_KEY_E = 69`).
    pub key_code:  i32,
    pub scan_code: i32,
    /// 0 = release, 1 = press, 2 = repeat.
    pub action:    i32,
    /// Modifier bitmask: 1=shift, 2=ctrl, 4=alt, 8=super.
    pub modifiers: i32,
}

// ── Handler function-pointer types ────────────────────────────────────────────
//
// All event handlers receive a `phase: u8` argument:
//   0 = Pre  — fires before the action; return value matters (false = cancel).
//   1 = Post — fires after the action; return value is ignored.
//
// This unified signature lets one registered closure handle both phases.
//
// Client-side handlers (minor 10) do NOT receive a `YogServer*` — they run on
// the render thread and have no server context.

pub type YogBlockBreakFn   = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogBlockBreakEvent,   u8) -> bool;
pub type YogChatFn         = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogChatEvent,         u8) -> bool;
pub type YogPlayerFn       = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogPlayerEvent,       u8) -> bool;
pub type YogUseItemFn      = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogUseItemEvent,      u8) -> bool;
pub type YogUseBlockFn     = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogUseBlockEvent,     u8) -> bool;
pub type YogAttackEntityFn = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogAttackEntityEvent, u8) -> bool;
pub type YogEntityDamageFn = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogEntityDamageEvent, u8) -> bool;
pub type YogEntityDeathFn  = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogEntityDeathEvent,  u8) -> bool;
pub type YogEntitySpawnFn   = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogEntitySpawnEvent,   u8) -> bool;
pub type YogPlaceBlockFn    = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogPlaceBlockEvent,    u8) -> bool;
pub type YogPlayerDeathFn   = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogPlayerDeathEvent,   u8) -> bool;
pub type YogPlayerRespawnFn = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogPlayerRespawnEvent, u8) -> bool;
pub type YogAdvancementFn      = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogAdvancementEvent,      u8) -> bool;
pub type YogEntityInteractFn   = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogEntityInteractEvent,   u8) -> bool;
pub type YogCraftFn            = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogCraftEvent,            u8) -> bool;
pub type YogExplosionFn        = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogExplosionEvent,        u8) -> bool;
pub type YogItemPickupFn       = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogItemPickupEvent,       u8) -> bool;
pub type YogPlayerMoveFn       = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogPlayerMoveEvent,       u8) -> bool;
pub type YogContainerOpenFn    = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogContainerOpenEvent,    u8) -> bool;
pub type YogContainerCloseFn   = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogContainerCloseEvent,   u8) -> bool;
pub type YogProjectileHitFn    = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogProjectileHitEvent,    u8) -> bool;

/// Packet events — always Post, no phase.
pub type YogPacketFn  = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogPacketEvent);
/// Server lifecycle / tick — no event struct, always fires.
pub type YogServerFn  = unsafe extern "C" fn(*mut c_void, *const YogServer);
/// Command handler.
pub type YogCommandFn = unsafe extern "C" fn(
    ud: *mut c_void,
    srv: *const YogServer,
    ev: *const YogCommandEvent,
    reply_buf: *mut u8,
    reply_cap: u32,
    reply_len: *mut u32,
);
/// Scheduler handler (once or repeating).
pub type YogScheduledFn = unsafe extern "C" fn(*mut c_void, *const YogServer);

// ── ABI minor 10 — client-side function pointer types ────────────────────────

/// Client tick — no event, no server context.
pub type YogClientFn    = unsafe extern "C" fn(ud: *mut c_void);
/// HUD render — `delta_tick` is the partial tick interpolation factor (0.0–1.0).
pub type YogHudRenderFn = unsafe extern "C" fn(ud: *mut c_void, delta_tick: f32);
/// Key press — return `false` to cancel (prevent Minecraft from processing the key).
pub type YogKeyPressFn  = unsafe extern "C" fn(ud: *mut c_void, ev: *const YogKeyPressEvent) -> bool;
/// Screen event — `screen_class` is the simple class name (e.g. `"InventoryScreen"`).
/// For `on_screen_open` return `false` to prevent the screen from opening.
pub type YogScreenFn    = unsafe extern "C" fn(ud: *mut c_void, screen_class: YogStr) -> bool;

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
    pub ctx:         *mut c_void,
    pub abi_version: u32,
    /// `sizeof(YogServer)` at build time — allows mods compiled against an older
    /// table to detect and skip fields they don't know about.
    pub size:        u32,

    /// Free a string returned by any function in this table.
    pub free_str: unsafe extern "C" fn(ptr: *mut u8, len: u32),

    // ── chat ─────────────────────────────────────────────────────────────────
    pub broadcast: unsafe extern "C" fn(ctx: *mut c_void, msg: YogStr),

    // ── world ────────────────────────────────────────────────────────────────
    pub get_block:   unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> YogOwnedStr,
    pub set_block:   unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogBlockPos, block: YogStr) -> bool,
    pub world_time:  unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, out: *mut i64) -> bool,
    pub set_time:    unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, time: i64) -> bool,
    pub is_raining:  unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr) -> bool,
    pub set_weather: unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, raining: bool, dur: i32) -> bool,

    // ── player ───────────────────────────────────────────────────────────────
    pub give_item:         unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, item: YogStr, count: u32) -> bool,
    pub player_teleport:   unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, pos: YogVec3) -> bool,
    pub send_to_player:    unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, channel: YogStr, data: *const u8, len: u32) -> bool,
    pub send_to_server:    unsafe extern "C" fn(ctx: *mut c_void, channel: YogStr, data: *const u8, len: u32) -> bool,
    pub kick_player:       unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, reason: YogStr) -> bool,
    pub set_gamemode:      unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, mode: YogStr) -> bool,
    pub send_title:        unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, title: YogStr, sub: YogStr, fi: i32, stay: i32, fo: i32) -> bool,
    pub send_actionbar:    unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, msg: YogStr) -> bool,
    pub play_sound:        unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogVec3, sound: YogStr, vol: f32, pitch: f32) -> bool,
    pub play_sound_player: unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, sound: YogStr, vol: f32, pitch: f32) -> bool,

    // ── entity ───────────────────────────────────────────────────────────────
    pub entity_teleport:      unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, pos: YogVec3) -> bool,
    pub entity_position:      unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, out: *mut YogVec3) -> bool,
    pub entity_health:        unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, out: *mut f32) -> bool,
    pub entity_set_health:    unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, hp: f32) -> bool,
    pub entity_kill:          unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr) -> bool,
    pub spawn_entity:         unsafe extern "C" fn(ctx: *mut c_void, type_id: YogStr, dim: YogStr, pos: YogVec3) -> YogOwnedStr,
    pub entity_add_effect:    unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, fx: YogStr, dur: i32, amp: u8, particles: bool) -> bool,
    pub entity_remove_effect: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, fx: YogStr) -> bool,
    pub entity_clear_effects: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr) -> bool,
    pub entity_velocity:      unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, out: *mut YogVec3) -> bool,
    pub entity_set_velocity:  unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, vel: YogVec3) -> bool,
    pub entity_add_velocity:  unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, vel: YogVec3) -> bool,

    // ── tags & loot ──────────────────────────────────────────────────────────
    pub has_item_tag:  unsafe extern "C" fn(ctx: *mut c_void, item: YogStr, tag: YogStr) -> bool,
    pub has_block_tag: unsafe extern "C" fn(ctx: *mut c_void, block: YogStr, tag: YogStr) -> bool,
    pub drop_loot:     unsafe extern "C" fn(ctx: *mut c_void, table: YogStr, dim: YogStr, pos: YogVec3) -> bool,

    // ── scoreboard ───────────────────────────────────────────────────────────
    pub scoreboard_get: unsafe extern "C" fn(ctx: *mut c_void, obj: YogStr, player: YogStr, out: *mut i32) -> bool,
    pub scoreboard_set: unsafe extern "C" fn(ctx: *mut c_void, obj: YogStr, player: YogStr, score: i32) -> bool,
    pub scoreboard_add: unsafe extern "C" fn(ctx: *mut c_void, obj: YogStr, player: YogStr, delta: i32, out: *mut i32) -> bool,

    // ── boss bar ─────────────────────────────────────────────────────────────
    pub bossbar_create:        unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, title: YogStr, color: YogStr, style: YogStr) -> bool,
    pub bossbar_remove:        unsafe extern "C" fn(ctx: *mut c_void, id: YogStr) -> bool,
    pub bossbar_set_title:     unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, title: YogStr) -> bool,
    pub bossbar_set_progress:  unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, progress: f32) -> bool,
    pub bossbar_set_color:     unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, color: YogStr) -> bool,
    pub bossbar_add_player:    unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, player: YogStr) -> bool,
    pub bossbar_remove_player: unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, player: YogStr) -> bool,
    pub bossbar_set_visible:   unsafe extern "C" fn(ctx: *mut c_void, id: YogStr, visible: bool) -> bool,

    // ── misc ─────────────────────────────────────────────────────────────────
    pub game_dir: unsafe extern "C" fn(ctx: *mut c_void) -> YogOwnedStr,

    // ── player query (ABI minor 4) ────────────────────────────────────────────
    /// Newline-separated list of online player names, or NONE if server not up.
    pub online_players: unsafe extern "C" fn(ctx: *mut c_void) -> YogOwnedStr,

    // ── block entity (NBT, ABI minor 3) ──────────────────────────────────────
    /// SNBT string of the block entity at `pos`, or NONE if there is none.
    pub get_block_nbt: unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> YogOwnedStr,
    /// Write SNBT into the block entity at `pos`. Returns false if no block entity exists.
    pub set_block_nbt: unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, pos: YogBlockPos, snbt: YogStr) -> bool,

    // ── inventory (ABI minor 3) ───────────────────────────────────────────────
    /// Tab/newline-encoded inventory: one line per occupied slot, `slot\titem_id\tcount`.
    pub player_inventory: unsafe extern "C" fn(ctx: *mut c_void, player: YogStr) -> YogOwnedStr,
    /// Set (or clear when count==0) a specific inventory slot.
    pub player_set_slot:  unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, slot: u32, item_id: YogStr, count: u32) -> bool,

    // ── cross-dimension teleport (ABI minor 3) ────────────────────────────────
    pub player_teleport_dim: unsafe extern "C" fn(ctx: *mut c_void, player: YogStr, dim: YogStr, pos: YogVec3) -> bool,
    pub entity_teleport_dim: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, dim: YogStr, pos: YogVec3) -> bool,

    // ── entity counting ───────────────────────────────────────────────────────
    /// Count loaded instances of `entity_type` in `dimension`. Returns -1 on error.
    pub world_entity_count: unsafe extern "C" fn(ctx: *mut c_void, dim: YogStr, entity_type: YogStr) -> i32,

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
        dx: f64, dy: f64, dz: f64,
        speed: f64,
    ) -> bool,

    // ── attributes (ABI minor 7) ──────────────────────────────────────────────
    /// Get the base value of an attribute on a living entity.
    /// `attribute_id` is a registry id, e.g. `"minecraft:generic.max_health"`.
    /// Returns `f64::NAN` if entity or attribute is not found.
    pub entity_attribute_get: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, attribute_id: YogStr) -> f64,
    /// Set the base value of an attribute. Returns false if entity or attribute is not found.
    pub entity_attribute_set: unsafe extern "C" fn(ctx: *mut c_void, uuid: YogStr, attribute_id: YogStr, value: f64) -> bool,
}

// ctx = *mut JavaVM which is global/stable. All fn ptrs are pure C-ABI.
unsafe impl Send for YogServer {}
unsafe impl Sync for YogServer {}

// ── Registration table (passed to yog_mod_register) ──────────────────────────

/// Passed to `yog_mod_register`. Call the function pointers here to register
/// handlers, commands, content, and schedulers.
///
/// When mods compiled against ABI `N` load on a runtime with ABI `M > N`:
/// fields beyond `size` are not present in the mod's view — check `size` before
/// accessing fields added in later minor versions.
#[repr(C)]
pub struct YogApi {
    pub abi_version: u32,
    /// `sizeof(YogApi)` at the runtime's build time.
    pub size:        u32,
    /// Opaque pointer to runtime handler storage.
    pub ctx:         *mut c_void,
    /// Stable server action table — pass to handlers.
    pub server:      *const YogServer,

    // ── events — all handlers receive (ud, srv, event, phase: u8) → bool ────────
    // phase 0 = Pre (return false to cancel), phase 1 = Post (return ignored).
    pub on_block_break:       unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogBlockBreakFn),
    pub on_chat:              unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogChatFn),
    pub on_player_join:       unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogPlayerFn),
    pub on_player_leave:      unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogPlayerFn),
    pub on_use_item:          unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogUseItemFn),
    pub on_use_block:         unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogUseBlockFn),
    pub on_attack_entity:     unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogAttackEntityFn),
    pub on_entity_damage:     unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogEntityDamageFn),
    pub on_entity_death:      unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogEntityDeathFn),
    pub on_entity_spawn:       unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogEntitySpawnFn),
    pub on_player_place_block: unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogPlaceBlockFn),
    pub on_player_death:       unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogPlayerDeathFn),
    pub on_player_respawn:     unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogPlayerRespawnFn),
    pub on_advancement:        unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogAdvancementFn),
    // ── ABI minor 8 ──────────────────────────────────────────────────────────
    pub on_entity_interact:    unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogEntityInteractFn),
    pub on_item_craft:         unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogCraftFn),
    pub on_explosion:          unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogExplosionFn),
    // ── ABI minor 9 ──────────────────────────────────────────────────────────
    pub on_item_pickup:        unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogItemPickupFn),
    pub on_player_move:        unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogPlayerMoveFn),
    pub on_container_open:     unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogContainerOpenFn),
    pub on_container_close:    unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogContainerCloseFn),
    pub on_projectile_hit:     unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogProjectileHitFn),
    pub on_server_tick:       unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogServerFn),
    pub on_server_started:    unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogServerFn),
    pub on_server_stopping:   unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogServerFn),

    // ── networking ───────────────────────────────────────────────────────────
    pub on_packet:        unsafe extern "C" fn(ctx: *mut c_void, channel: YogStr, ud: *mut c_void, h: YogPacketFn),
    pub on_client_packet: unsafe extern "C" fn(ctx: *mut c_void, channel: YogStr, ud: *mut c_void, h: YogPacketFn),

    // ── commands ─────────────────────────────────────────────────────────────
    pub register_command: unsafe extern "C" fn(ctx: *mut c_void, name: YogStr, ud: *mut c_void, h: YogCommandFn),
    pub register_typed_command: unsafe extern "C" fn(ctx: *mut c_void, name: YogStr, schema: YogStr, ud: *mut c_void, h: YogCommandFn),

    // ── recipes ──────────────────────────────────────────────────────────────
    /// Register a recipe by supplying Minecraft JSON (`data/` format).
    /// `namespace` + `name` form the file path: `data/{ns}/recipes/{name}.json`.
    pub register_recipe_json: unsafe extern "C" fn(ctx: *mut c_void, namespace: YogStr, name: YogStr, json: YogStr),

    // ── content ──────────────────────────────────────────────────────────────
    pub register_item:  unsafe extern "C" fn(ctx: *mut c_void, def: *const YogItemDef),
    pub register_block: unsafe extern "C" fn(ctx: *mut c_void, def: *const YogBlockDef),

    // ── scheduler ────────────────────────────────────────────────────────────
    pub schedule_once:      unsafe extern "C" fn(ctx: *mut c_void, delay_ticks: u64, ud: *mut c_void, h: YogScheduledFn),
    pub schedule_repeating: unsafe extern "C" fn(ctx: *mut c_void, period_ticks: u64, ud: *mut c_void, h: YogScheduledFn),

    // ── ABI minor 10 — client-side events ────────────────────────────────────
    pub on_client_tick:  unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogClientFn),
    pub on_hud_render:   unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogHudRenderFn),
    pub on_key_press:    unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogKeyPressFn),
    pub on_screen_open:  unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogScreenFn),
    pub on_screen_close: unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogScreenFn),
}

unsafe impl Send for YogApi {}
unsafe impl Sync for YogApi {}
