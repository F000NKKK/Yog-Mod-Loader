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
pub const ABI_MINOR: u32 = 2;
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

// ── Handler function-pointer types ────────────────────────────────────────────

pub type YogBlockBreakFn   = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogBlockBreakEvent);
pub type YogChatFn         = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogChatEvent);
pub type YogPlayerFn       = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogPlayerEvent);
pub type YogUseItemFn      = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogUseItemEvent);
pub type YogUseBlockFn     = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogUseBlockEvent);
pub type YogAttackEntityFn = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogAttackEntityEvent);
pub type YogEntityDamageFn = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogEntityDamageEvent);
pub type YogEntityDeathFn  = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogEntityDeathEvent);
pub type YogPacketFn       = unsafe extern "C" fn(*mut c_void, *const YogServer, *const YogPacketEvent);
/// Tick / server-started / server-stopping handlers.
pub type YogServerFn       = unsafe extern "C" fn(*mut c_void, *const YogServer);
/// Command handler — writes reply into `reply_buf[0..reply_cap]` and sets `*reply_len`.
/// `*reply_len == 0` means no reply.
pub type YogCommandFn      = unsafe extern "C" fn(
    ud: *mut c_void,
    srv: *const YogServer,
    ev: *const YogCommandEvent,
    reply_buf: *mut u8,
    reply_cap: u32,
    reply_len: *mut u32,
);
/// Scheduler handler (once or repeating).
pub type YogScheduledFn = unsafe extern "C" fn(*mut c_void, *const YogServer);

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

    // ── events ───────────────────────────────────────────────────────────────
    pub on_block_break:     unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogBlockBreakFn),
    pub on_chat:            unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogChatFn),
    pub on_player_join:     unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogPlayerFn),
    pub on_player_leave:    unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogPlayerFn),
    pub on_use_item:        unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogUseItemFn),
    pub on_use_block:       unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogUseBlockFn),
    pub on_attack_entity:   unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogAttackEntityFn),
    pub on_entity_damage:   unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogEntityDamageFn),
    pub on_entity_death:    unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogEntityDeathFn),
    pub on_server_tick:     unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogServerFn),
    pub on_server_started:  unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogServerFn),
    pub on_server_stopping: unsafe extern "C" fn(ctx: *mut c_void, ud: *mut c_void, h: YogServerFn),

    // ── networking ───────────────────────────────────────────────────────────
    pub on_packet:        unsafe extern "C" fn(ctx: *mut c_void, channel: YogStr, ud: *mut c_void, h: YogPacketFn),
    pub on_client_packet: unsafe extern "C" fn(ctx: *mut c_void, channel: YogStr, ud: *mut c_void, h: YogPacketFn),

    // ── commands ─────────────────────────────────────────────────────────────
    pub register_command: unsafe extern "C" fn(ctx: *mut c_void, name: YogStr, ud: *mut c_void, h: YogCommandFn),

    // ── content ──────────────────────────────────────────────────────────────
    pub register_item:  unsafe extern "C" fn(ctx: *mut c_void, def: *const YogItemDef),
    pub register_block: unsafe extern "C" fn(ctx: *mut c_void, def: *const YogBlockDef),

    // ── scheduler ────────────────────────────────────────────────────────────
    pub schedule_once:      unsafe extern "C" fn(ctx: *mut c_void, delay_ticks: u64, ud: *mut c_void, h: YogScheduledFn),
    pub schedule_repeating: unsafe extern "C" fn(ctx: *mut c_void, period_ticks: u64, ud: *mut c_void, h: YogScheduledFn),
}

unsafe impl Send for YogApi {}
unsafe impl Sync for YogApi {}
