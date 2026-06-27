//! Yog runtime — the native library loaded by the Fabric host.
//!
//! Exposes JNI entry points (`Java_dev_yog_NativeBridge_*`) that the host calls,
//! and a stable C ABI (`YogApi` / `YogServer`) that mods program against.
//!
//! Architecture:
//!   - `YogServer`  — a `#[repr(C)]` table of standalone JNI-calling functions
//!                    that mods call to mutate the world.
//!   - `YogApi`     — a `#[repr(C)]` table of registration functions; mods call
//!                    them inside `yog_mod_register` to subscribe to events.
//!   - `RuntimeHandlers` — the runtime's internal event/handler storage.
//!     Filled during `nativeInit` (write), read-only after. The scheduler sub-
//!     state uses an inner `Mutex` for safe addition during event dispatch.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::os::raw::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use glow::HasContext;
use jni::objects::{JByteArray, JClass, JFloatArray, JObject, JString, JValue};
use jni::sys::{jdouble, jfloat, jint, jstring};
use jni::{JNIEnv, JavaVM};
use libloading::{Library, Symbol};

use yog_abi::{
    ABI_VERSION, YogAdvancementEvent, YogAdvancementFn, YogApi, YogAttackEntityFn,
    YogBlockBreakFn, YogBlockDef, YogBlockPos, YogChatFn, YogClientFn, YogCommandFn,
    YogContainerCloseEvent, YogContainerCloseFn, YogContainerOpenEvent, YogContainerOpenFn,
    YogCraftEvent, YogCraftFn, YogEntityDamageFn, YogEntityDeathFn, YogEntityInteractEvent,
    YogEntityInteractFn, YogEntitySpawnFn, YogExplosionEvent, YogExplosionFn,
    YogGfxApi, YogHudRenderFn, YogItemDef, YogItemPickupEvent, YogItemPickupFn,
    YogKeyPressFn, YogKeyPressEvent, YogOwnedStr, YogPacketFn, YogPlaceBlockEvent,
    YogPlaceBlockFn, YogPlayerDeathEvent, YogPlayerDeathFn, YogPlayerFn, YogPlayerMoveEvent,
    YogPlayerMoveFn, YogPlayerRespawnEvent, YogPlayerRespawnFn, YogProjectileHitEvent,
    YogProjectileHitFn, YogScheduledFn, YogScreenFn, YogServer, YogServerFn, YogStr, YogStartupGrantDef,
    YogUseBlockFn, YogUseItemFn, YogVec3, YogWorldRenderFn,
};
use yog_registry::{BlockDef, FoodDef, ItemDef};

// ── Static globals ────────────────────────────────────────────────────────────

/// Cached JVM handle for any-thread callbacks.
static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();
/// Loaded mod libraries — kept alive so the code pages stay mapped.
static LOADED_MODS: Mutex<Vec<Library>> = Mutex::new(Vec::new());
/// Stable server table (populated once in nativeInit, then read-only).
static SERVER: OnceLock<YogServer> = OnceLock::new();
/// All registered handlers + content (populated during mod loading, then read-only).
static HANDLERS: OnceLock<RuntimeHandlers> = OnceLock::new();

// ── OpenGL context (client-side, render thread only) ─────────────────────────

struct GlCtx(glow::Context);
unsafe impl Send for GlCtx {}
unsafe impl Sync for GlCtx {}

/// Initialized by `nativeGlInit` on the render thread.  `None` on dedicated server.
static GL: OnceLock<GlCtx> = OnceLock::new();

// Raw GL function pointers for GL_ARB_get_program_binary (not exposed by glow 0.13).
// Captured during the glow loader callback in `nativeGlInit`.
// `None` when the extension is unavailable (very old drivers).
static GL_GET_PROGRAM_BINARY: OnceLock<Option<usize>> = OnceLock::new();
static GL_PROGRAM_BINARY:     OnceLock<Option<usize>> = OnceLock::new();
static GL_GET_PROGRAM_IV:     OnceLock<Option<usize>> = OnceLock::new();

// ── Handler storage ───────────────────────────────────────────────────────────

struct RuntimeHandlers {
    block_break:        Vec<(*mut c_void, YogBlockBreakFn)>,
    chat:               Vec<(*mut c_void, YogChatFn)>,
    player_join:        Vec<(*mut c_void, YogPlayerFn)>,
    player_leave:       Vec<(*mut c_void, YogPlayerFn)>,
    use_item:           Vec<(*mut c_void, YogUseItemFn)>,
    use_block:          Vec<(*mut c_void, YogUseBlockFn)>,
    attack_entity:      Vec<(*mut c_void, YogAttackEntityFn)>,
    entity_damage:      Vec<(*mut c_void, YogEntityDamageFn)>,
    entity_death:       Vec<(*mut c_void, YogEntityDeathFn)>,
    entity_spawn:       Vec<(*mut c_void, YogEntitySpawnFn)>,
    player_place_block: Vec<(*mut c_void, YogPlaceBlockFn)>,
    player_death:       Vec<(*mut c_void, YogPlayerDeathFn)>,
    player_respawn:     Vec<(*mut c_void, YogPlayerRespawnFn)>,
    advancement:        Vec<(*mut c_void, YogAdvancementFn)>,
    entity_interact:    Vec<(*mut c_void, YogEntityInteractFn)>,
    item_craft:         Vec<(*mut c_void, YogCraftFn)>,
    explosion:          Vec<(*mut c_void, YogExplosionFn)>,
    item_pickup:        Vec<(*mut c_void, YogItemPickupFn)>,
    player_move:        Vec<(*mut c_void, YogPlayerMoveFn)>,
    container_open:     Vec<(*mut c_void, YogContainerOpenFn)>,
    container_close:    Vec<(*mut c_void, YogContainerCloseFn)>,
    projectile_hit:     Vec<(*mut c_void, YogProjectileHitFn)>,
    client_tick:        Vec<(*mut c_void, YogClientFn)>,
    hud_render:         Vec<(*mut c_void, YogHudRenderFn)>,
    world_render:       Vec<(*mut c_void, YogWorldRenderFn)>,
    key_press:          Vec<(*mut c_void, YogKeyPressFn)>,
    screen_open:        Vec<(*mut c_void, YogScreenFn)>,
    screen_close:       Vec<(*mut c_void, YogScreenFn)>,
    server_tick:        Vec<(*mut c_void, YogServerFn)>,
    server_started:     Vec<(*mut c_void, YogServerFn)>,
    server_stopping:    Vec<(*mut c_void, YogServerFn)>,
    commands:           HashMap<String, (*mut c_void, YogCommandFn)>,
    typed_schemas:      HashMap<String, String>,
    recipes:            Vec<(String, String, String)>,
    packets:            HashMap<String, (*mut c_void, YogPacketFn)>,
    client_packets:     HashMap<String, (*mut c_void, YogPacketFn)>,
    items:              Vec<ItemDef>,
    blocks:             Vec<BlockDef>,
    books:              HashMap<String, String>, // book_id → JSON
    pub(crate) uis:     HashMap<String, yog_ui::LayoutNode>, // ui_id → current layout
    ui_handlers:        HashMap<String, (*mut c_void, yog_abi::YogUIEventFn)>, // ui_id → callback
    startup_grants:     Vec<yog_registry::StartupGrant>,
    startup_granted:    Mutex<HashMap<String, bool>>,
    scheduler:          Mutex<SchedulerState>,
}

// All fn ptrs are C-ABI; ud pointers are from Box::into_raw of Send+Sync closures.
unsafe impl Send for RuntimeHandlers {}
unsafe impl Sync for RuntimeHandlers {}

impl RuntimeHandlers {
    fn new() -> Self {
        Self {
            block_break: Vec::new(), chat: Vec::new(),
            player_join: Vec::new(), player_leave: Vec::new(),
            use_item: Vec::new(), use_block: Vec::new(),
            attack_entity: Vec::new(), entity_damage: Vec::new(),
            entity_death: Vec::new(), entity_spawn: Vec::new(),
            player_place_block: Vec::new(),
            player_death: Vec::new(), player_respawn: Vec::new(), advancement: Vec::new(),
            entity_interact: Vec::new(), item_craft: Vec::new(), explosion: Vec::new(),
            item_pickup: Vec::new(), player_move: Vec::new(),
            container_open: Vec::new(), container_close: Vec::new(),
            projectile_hit: Vec::new(),
            client_tick: Vec::new(), hud_render: Vec::new(), world_render: Vec::new(),
            key_press: Vec::new(),
            screen_open: Vec::new(), screen_close: Vec::new(),
            server_tick: Vec::new(), server_started: Vec::new(), server_stopping: Vec::new(),
            commands: HashMap::new(), typed_schemas: HashMap::new(),
            recipes: Vec::new(), packets: HashMap::new(),
            client_packets: HashMap::new(), items: Vec::new(),
            blocks: Vec::new(), books: HashMap::new(), uis: HashMap::new(),
            ui_handlers: HashMap::new(), startup_grants: Vec::new(),
            startup_granted: Mutex::new(HashMap::new()),
            scheduler: Mutex::new(SchedulerState::new()),
        }
    }
}

struct SchedulerState {
    once_tasks:       Vec<OnceTask>,
    repeating_tasks:  Vec<RepeatingTask>,
}

struct OnceTask      { delay_remaining: u64, ud: *mut c_void, f: YogScheduledFn }
struct RepeatingTask { period: u64, ticks_left: u64, ud: *mut c_void, f: YogScheduledFn }

unsafe impl Send for SchedulerState {}
unsafe impl Sync for SchedulerState {}
unsafe impl Send for OnceTask {}
unsafe impl Send for RepeatingTask {}

impl SchedulerState {
    fn new() -> Self { Self { once_tasks: Vec::new(), repeating_tasks: Vec::new() } }
}

fn handlers() -> &'static RuntimeHandlers {
    HANDLERS.get().expect("yog: nativeInit not called yet")
}

// ── JNI helpers ──────────────────────────────────────────────────────────────

fn guard(label: &str, f: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err() {
        yog_logging::error!("a mod panicked handling `{}` (ignored)", label);
    }
}

macro_rules! jstr {
    ($env:expr, $s:expr) => {
        match $env.get_string(&$s) { Ok(s) => String::from(s), Err(_) => return }
    };
}

/// Convert a `YogStr` into a Java String. Caller must ensure `s` is valid UTF-8.
unsafe fn ys_to_java<'l>(env: &mut JNIEnv<'l>, s: YogStr)
    -> Option<jni::objects::JString<'l>>
{
    env.new_string(s.as_str()).ok()
}

fn get_env() -> Option<jni::AttachGuard<'static>> {
    JAVA_VM.get()?.attach_current_thread().ok()
}

// ── Free-str allocator used by YogOwnedStr ────────────────────────────────────

unsafe extern "C" fn yog_free_str(ptr: *mut u8, len: u32) {
    if !ptr.is_null() {
        drop(Box::from_raw(std::slice::from_raw_parts_mut(ptr, len as usize)));
    }
}

fn jstring_to_owned(env: &mut JNIEnv, obj: jni::objects::JObject) -> YogOwnedStr {
    if obj.as_raw().is_null() { return YogOwnedStr::NONE; }
    match env.get_string(&JString::from(obj)) {
        Ok(s) => YogOwnedStr::from_string(String::from(s)),
        Err(_) => YogOwnedStr::NONE,
    }
}

// ── YogServer standalone functions (one per action) ───────────────────────────
//
// ctx is unused here — all state is in the JAVA_VM static.

unsafe extern "C" fn srv_broadcast(_ctx: *mut c_void, msg: YogStr) {
    let Some(mut env) = get_env() else { return };
    if let Some(jmsg) = ys_to_java(&mut env, msg) {
        let _ = env.call_static_method("dev/yog/NativeBridge", "broadcast",
            "(Ljava/lang/String;)V", &[JValue::Object(&jmsg)]);
    }
}

unsafe extern "C" fn srv_get_block(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> YogOwnedStr {
    let Some(mut env) = get_env() else { return YogOwnedStr::NONE };
    let (Some(jd), ) = (ys_to_java(&mut env, dim),) else { return YogOwnedStr::NONE };
    let ret = env.call_static_method("dev/yog/NativeBridge", "getBlock",
        "(Ljava/lang/String;III)Ljava/lang/String;",
        &[JValue::Object(&jd), JValue::Int(pos.x), JValue::Int(pos.y), JValue::Int(pos.z)]);
    match ret.and_then(|v| v.l()) {
        Ok(obj) => jstring_to_owned(&mut env, obj),
        _ => YogOwnedStr::NONE,
    }
}

unsafe extern "C" fn srv_set_block(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos, block: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jd), Some(jb)) = (ys_to_java(&mut env, dim), ys_to_java(&mut env, block)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "setBlock",
        "(Ljava/lang/String;IIILjava/lang/String;)Z",
        &[JValue::Object(&jd), JValue::Int(pos.x), JValue::Int(pos.y), JValue::Int(pos.z), JValue::Object(&jb)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_world_time(_ctx: *mut c_void, dim: YogStr, out: *mut i64) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(jd) = ys_to_java(&mut env, dim) else { return false };
    match env.call_static_method("dev/yog/NativeBridge", "worldTime",
        "(Ljava/lang/String;)J", &[JValue::Object(&jd)]).and_then(|v| v.j()) {
        Ok(v) if v != i64::MIN => { *out = v; true }
        _ => false,
    }
}

unsafe extern "C" fn srv_set_time(_ctx: *mut c_void, dim: YogStr, time: i64) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(jd) = ys_to_java(&mut env, dim) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "worldSetTime",
        "(Ljava/lang/String;J)Z", &[JValue::Object(&jd), JValue::Long(time)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_is_raining(_ctx: *mut c_void, dim: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(jd) = ys_to_java(&mut env, dim) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "worldIsRaining",
        "(Ljava/lang/String;)Z", &[JValue::Object(&jd)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_set_weather(_ctx: *mut c_void, dim: YogStr, raining: bool, dur: i32) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(jd) = ys_to_java(&mut env, dim) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "worldSetWeather",
        "(Ljava/lang/String;ZI)Z",
        &[JValue::Object(&jd), JValue::Bool(raining as u8), JValue::Int(dur)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_give_item(_ctx: *mut c_void, player: YogStr, item: YogStr, count: u32) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jp), Some(ji)) = (ys_to_java(&mut env, player), ys_to_java(&mut env, item)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "giveItem",
        "(Ljava/lang/String;Ljava/lang/String;I)Z",
        &[JValue::Object(&jp), JValue::Object(&ji), JValue::Int(count as i32)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_player_teleport(_ctx: *mut c_void, player: YogStr, pos: YogVec3) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(jp) = ys_to_java(&mut env, player) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "teleport",
        "(Ljava/lang/String;DDD)Z",
        &[JValue::Object(&jp), JValue::Double(pos.x), JValue::Double(pos.y), JValue::Double(pos.z)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_send_to_player(_ctx: *mut c_void, player: YogStr, channel: YogStr, data: *const u8, len: u32) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jp), Some(jc)) = (ys_to_java(&mut env, player), ys_to_java(&mut env, channel)) else { return false };
    let payload = std::slice::from_raw_parts(data, len as usize);
    let Ok(jdata) = env.byte_array_from_slice(payload) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "sendToPlayer",
        "(Ljava/lang/String;Ljava/lang/String;[B)Z",
        &[JValue::Object(&jp), JValue::Object(&jc), JValue::Object(&jdata)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_send_to_server(_ctx: *mut c_void, channel: YogStr, data: *const u8, len: u32) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(jc) = ys_to_java(&mut env, channel) else { return false };
    let payload = std::slice::from_raw_parts(data, len as usize);
    let Ok(jdata) = env.byte_array_from_slice(payload) else { return false };
    let result = env.call_static_method("dev/yog/YogClient", "sendToServer",
        "(Ljava/lang/String;[B)Z", &[JValue::Object(&jc), JValue::Object(&jdata)]);
    let _ = env.exception_clear();
    result.and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_kick_player(_ctx: *mut c_void, player: YogStr, reason: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jp), Some(jr)) = (ys_to_java(&mut env, player), ys_to_java(&mut env, reason)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "kickPlayer",
        "(Ljava/lang/String;Ljava/lang/String;)Z", &[JValue::Object(&jp), JValue::Object(&jr)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_set_gamemode(_ctx: *mut c_void, player: YogStr, mode: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jp), Some(jg)) = (ys_to_java(&mut env, player), ys_to_java(&mut env, mode)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "setGamemode",
        "(Ljava/lang/String;Ljava/lang/String;)Z", &[JValue::Object(&jp), JValue::Object(&jg)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_send_title(_ctx: *mut c_void, player: YogStr, title: YogStr, sub: YogStr, fi: i32, stay: i32, fo: i32) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jp), Some(jt), Some(js)) = (ys_to_java(&mut env, player), ys_to_java(&mut env, title), ys_to_java(&mut env, sub)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "sendTitle",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;III)Z",
        &[JValue::Object(&jp), JValue::Object(&jt), JValue::Object(&js), JValue::Int(fi), JValue::Int(stay), JValue::Int(fo)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_send_actionbar(_ctx: *mut c_void, player: YogStr, msg: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jp), Some(jm)) = (ys_to_java(&mut env, player), ys_to_java(&mut env, msg)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "sendActionbar",
        "(Ljava/lang/String;Ljava/lang/String;)Z", &[JValue::Object(&jp), JValue::Object(&jm)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_play_sound(_ctx: *mut c_void, dim: YogStr, pos: YogVec3, sound: YogStr, vol: f32, pitch: f32) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jd), Some(js)) = (ys_to_java(&mut env, dim), ys_to_java(&mut env, sound)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "playSound",
        "(Ljava/lang/String;DDDLjava/lang/String;FF)Z",
        &[JValue::Object(&jd), JValue::Double(pos.x), JValue::Double(pos.y), JValue::Double(pos.z), JValue::Object(&js), JValue::Float(vol), JValue::Float(pitch)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_play_sound_player(_ctx: *mut c_void, player: YogStr, sound: YogStr, vol: f32, pitch: f32) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jp), Some(js)) = (ys_to_java(&mut env, player), ys_to_java(&mut env, sound)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "playSoundToPlayer",
        "(Ljava/lang/String;Ljava/lang/String;FF)Z",
        &[JValue::Object(&jp), JValue::Object(&js), JValue::Float(vol), JValue::Float(pitch)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_entity_teleport(_ctx: *mut c_void, uuid: YogStr, pos: YogVec3) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(ju) = ys_to_java(&mut env, uuid) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "entityTeleport",
        "(Ljava/lang/String;DDD)Z",
        &[JValue::Object(&ju), JValue::Double(pos.x), JValue::Double(pos.y), JValue::Double(pos.z)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_entity_position(_ctx: *mut c_void, uuid: YogStr, out: *mut YogVec3) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(ju) = ys_to_java(&mut env, uuid) else { return false };
    let ret = env.call_static_method("dev/yog/NativeBridge", "entityPosition",
        "(Ljava/lang/String;)Ljava/lang/String;", &[JValue::Object(&ju)]);
    let obj = match ret.and_then(|v| v.l()) { Ok(o) => o, Err(_) => return false };
    if obj.as_raw().is_null() { return false; }
    let s: String = match env.get_string(&JString::from(obj)) { Ok(s) => String::from(s), Err(_) => return false };
    let mut it = s.split('\t');
    let (x, y, z) = (it.next(), it.next(), it.next());
    if let (Some(x), Some(y), Some(z)) = (x.and_then(|v| v.parse().ok()), y.and_then(|v| v.parse().ok()), z.and_then(|v| v.parse().ok())) {
        *out = YogVec3 { x, y, z }; true
    } else { false }
}

unsafe extern "C" fn srv_entity_health(_ctx: *mut c_void, uuid: YogStr, out: *mut f32) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(ju) = ys_to_java(&mut env, uuid) else { return false };
    match env.call_static_method("dev/yog/NativeBridge", "entityHealth",
        "(Ljava/lang/String;)D", &[JValue::Object(&ju)]).and_then(|v| v.d()) {
        Ok(v) if !v.is_nan() => { *out = v as f32; true }
        _ => false,
    }
}

unsafe extern "C" fn srv_entity_set_health(_ctx: *mut c_void, uuid: YogStr, hp: f32) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(ju) = ys_to_java(&mut env, uuid) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "entitySetHealth",
        "(Ljava/lang/String;D)Z", &[JValue::Object(&ju), JValue::Double(hp as f64)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_entity_kill(_ctx: *mut c_void, uuid: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(ju) = ys_to_java(&mut env, uuid) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "entityKill",
        "(Ljava/lang/String;)Z", &[JValue::Object(&ju)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_spawn_entity(_ctx: *mut c_void, type_id: YogStr, dim: YogStr, pos: YogVec3) -> YogOwnedStr {
    let Some(mut env) = get_env() else { return YogOwnedStr::NONE };
    let (Some(jt), Some(jd)) = (ys_to_java(&mut env, type_id), ys_to_java(&mut env, dim)) else { return YogOwnedStr::NONE };
    let ret = env.call_static_method("dev/yog/NativeBridge", "spawnEntity",
        "(Ljava/lang/String;Ljava/lang/String;DDD)Ljava/lang/String;",
        &[JValue::Object(&jt), JValue::Object(&jd), JValue::Double(pos.x), JValue::Double(pos.y), JValue::Double(pos.z)]);
    match ret.and_then(|v| v.l()) {
        Ok(obj) => jstring_to_owned(&mut env, obj),
        _ => YogOwnedStr::NONE,
    }
}

unsafe extern "C" fn srv_entity_add_effect(_ctx: *mut c_void, uuid: YogStr, fx: YogStr, dur: i32, amp: u8, particles: bool) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(ju), Some(je)) = (ys_to_java(&mut env, uuid), ys_to_java(&mut env, fx)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "entityAddEffect",
        "(Ljava/lang/String;Ljava/lang/String;IIZ)Z",
        &[JValue::Object(&ju), JValue::Object(&je), JValue::Int(dur), JValue::Int(amp as i32), JValue::Bool(particles as u8)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_entity_remove_effect(_ctx: *mut c_void, uuid: YogStr, fx: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(ju), Some(je)) = (ys_to_java(&mut env, uuid), ys_to_java(&mut env, fx)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "entityRemoveEffect",
        "(Ljava/lang/String;Ljava/lang/String;)Z", &[JValue::Object(&ju), JValue::Object(&je)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_entity_clear_effects(_ctx: *mut c_void, uuid: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(ju) = ys_to_java(&mut env, uuid) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "entityClearEffects",
        "(Ljava/lang/String;)Z", &[JValue::Object(&ju)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_entity_velocity(_ctx: *mut c_void, uuid: YogStr, out: *mut YogVec3) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(ju) = ys_to_java(&mut env, uuid) else { return false };
    let ret = env.call_static_method("dev/yog/NativeBridge", "entityVelocity",
        "(Ljava/lang/String;)Ljava/lang/String;", &[JValue::Object(&ju)]);
    let obj = match ret.and_then(|v| v.l()) { Ok(o) => o, Err(_) => return false };
    if obj.as_raw().is_null() { return false; }
    let s: String = match env.get_string(&JString::from(obj)) { Ok(s) => String::from(s), Err(_) => return false };
    let mut it = s.split('\t');
    let (x, y, z) = (it.next(), it.next(), it.next());
    if let (Some(x), Some(y), Some(z)) = (x.and_then(|v| v.parse().ok()), y.and_then(|v| v.parse().ok()), z.and_then(|v| v.parse().ok())) {
        *out = YogVec3 { x, y, z }; true
    } else { false }
}

unsafe extern "C" fn srv_entity_set_velocity(_ctx: *mut c_void, uuid: YogStr, vel: YogVec3) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(ju) = ys_to_java(&mut env, uuid) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "entitySetVelocity",
        "(Ljava/lang/String;DDD)Z",
        &[JValue::Object(&ju), JValue::Double(vel.x), JValue::Double(vel.y), JValue::Double(vel.z)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_entity_add_velocity(_ctx: *mut c_void, uuid: YogStr, vel: YogVec3) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(ju) = ys_to_java(&mut env, uuid) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "entityAddVelocity",
        "(Ljava/lang/String;DDD)Z",
        &[JValue::Object(&ju), JValue::Double(vel.x), JValue::Double(vel.y), JValue::Double(vel.z)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_has_item_tag(_ctx: *mut c_void, item: YogStr, tag: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(ji), Some(jt)) = (ys_to_java(&mut env, item), ys_to_java(&mut env, tag)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "hasItemTag",
        "(Ljava/lang/String;Ljava/lang/String;)Z", &[JValue::Object(&ji), JValue::Object(&jt)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_has_block_tag(_ctx: *mut c_void, block: YogStr, tag: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jb), Some(jt)) = (ys_to_java(&mut env, block), ys_to_java(&mut env, tag)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "hasBlockTag",
        "(Ljava/lang/String;Ljava/lang/String;)Z", &[JValue::Object(&jb), JValue::Object(&jt)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_drop_loot(_ctx: *mut c_void, table: YogStr, dim: YogStr, pos: YogVec3) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jt), Some(jd)) = (ys_to_java(&mut env, table), ys_to_java(&mut env, dim)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "dropLoot",
        "(Ljava/lang/String;Ljava/lang/String;DDD)Z",
        &[JValue::Object(&jt), JValue::Object(&jd), JValue::Double(pos.x), JValue::Double(pos.y), JValue::Double(pos.z)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_scoreboard_get(_ctx: *mut c_void, obj: YogStr, player: YogStr, out: *mut i32) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jo), Some(jp)) = (ys_to_java(&mut env, obj), ys_to_java(&mut env, player)) else { return false };
    match env.call_static_method("dev/yog/NativeBridge", "scoreboardGet",
        "(Ljava/lang/String;Ljava/lang/String;)I", &[JValue::Object(&jo), JValue::Object(&jp)]).and_then(|v| v.i()) {
        Ok(v) if v != i32::MIN => { *out = v; true }
        _ => false,
    }
}

unsafe extern "C" fn srv_scoreboard_set(_ctx: *mut c_void, obj: YogStr, player: YogStr, score: i32) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jo), Some(jp)) = (ys_to_java(&mut env, obj), ys_to_java(&mut env, player)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "scoreboardSet",
        "(Ljava/lang/String;Ljava/lang/String;I)Z",
        &[JValue::Object(&jo), JValue::Object(&jp), JValue::Int(score)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_scoreboard_add(_ctx: *mut c_void, obj: YogStr, player: YogStr, delta: i32, out: *mut i32) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jo), Some(jp)) = (ys_to_java(&mut env, obj), ys_to_java(&mut env, player)) else { return false };
    match env.call_static_method("dev/yog/NativeBridge", "scoreboardAdd",
        "(Ljava/lang/String;Ljava/lang/String;I)I",
        &[JValue::Object(&jo), JValue::Object(&jp), JValue::Int(delta)]).and_then(|v| v.i()) {
        Ok(v) if v != i32::MIN => { *out = v; true }
        _ => false,
    }
}

unsafe extern "C" fn srv_bossbar_create(_ctx: *mut c_void, id: YogStr, title: YogStr, color: YogStr, style: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(ji), Some(jt), Some(jc), Some(js)) = (ys_to_java(&mut env, id), ys_to_java(&mut env, title), ys_to_java(&mut env, color), ys_to_java(&mut env, style)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "bossbarCreate",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Z",
        &[JValue::Object(&ji), JValue::Object(&jt), JValue::Object(&jc), JValue::Object(&js)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_bossbar_remove(_ctx: *mut c_void, id: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(ji) = ys_to_java(&mut env, id) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "bossbarRemove",
        "(Ljava/lang/String;)Z", &[JValue::Object(&ji)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_bossbar_set_title(_ctx: *mut c_void, id: YogStr, title: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(ji), Some(jt)) = (ys_to_java(&mut env, id), ys_to_java(&mut env, title)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "bossbarSetTitle",
        "(Ljava/lang/String;Ljava/lang/String;)Z", &[JValue::Object(&ji), JValue::Object(&jt)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_bossbar_set_progress(_ctx: *mut c_void, id: YogStr, progress: f32) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(ji) = ys_to_java(&mut env, id) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "bossbarSetProgress",
        "(Ljava/lang/String;F)Z", &[JValue::Object(&ji), JValue::Float(progress)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_bossbar_set_color(_ctx: *mut c_void, id: YogStr, color: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(ji), Some(jc)) = (ys_to_java(&mut env, id), ys_to_java(&mut env, color)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "bossbarSetColor",
        "(Ljava/lang/String;Ljava/lang/String;)Z", &[JValue::Object(&ji), JValue::Object(&jc)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_bossbar_add_player(_ctx: *mut c_void, id: YogStr, player: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(ji), Some(jp)) = (ys_to_java(&mut env, id), ys_to_java(&mut env, player)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "bossbarAddPlayer",
        "(Ljava/lang/String;Ljava/lang/String;)Z", &[JValue::Object(&ji), JValue::Object(&jp)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_bossbar_remove_player(_ctx: *mut c_void, id: YogStr, player: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(ji), Some(jp)) = (ys_to_java(&mut env, id), ys_to_java(&mut env, player)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "bossbarRemovePlayer",
        "(Ljava/lang/String;Ljava/lang/String;)Z", &[JValue::Object(&ji), JValue::Object(&jp)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_bossbar_set_visible(_ctx: *mut c_void, id: YogStr, visible: bool) -> bool {
    let Some(mut env) = get_env() else { return false };
    let Some(ji) = ys_to_java(&mut env, id) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "bossbarSetVisible",
        "(Ljava/lang/String;Z)Z", &[JValue::Object(&ji), JValue::Bool(visible as u8)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_get_block_nbt(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos) -> YogOwnedStr {
    let Some(mut env) = get_env() else { return YogOwnedStr::NONE };
    let Some(jd) = ys_to_java(&mut env, dim) else { return YogOwnedStr::NONE };
    let ret = env.call_static_method("dev/yog/NativeBridge", "getBlockNbt",
        "(Ljava/lang/String;III)Ljava/lang/String;",
        &[JValue::Object(&jd), JValue::Int(pos.x), JValue::Int(pos.y), JValue::Int(pos.z)]);
    match ret.and_then(|v| v.l()) {
        Ok(obj) => jstring_to_owned(&mut env, obj),
        _ => YogOwnedStr::NONE,
    }
}

unsafe extern "C" fn srv_set_block_nbt(_ctx: *mut c_void, dim: YogStr, pos: YogBlockPos, snbt: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jd), Some(js)) = (ys_to_java(&mut env, dim), ys_to_java(&mut env, snbt)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "setBlockNbt",
        "(Ljava/lang/String;IIILjava/lang/String;)Z",
        &[JValue::Object(&jd), JValue::Int(pos.x), JValue::Int(pos.y), JValue::Int(pos.z), JValue::Object(&js)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_player_inventory(_ctx: *mut c_void, player: YogStr) -> YogOwnedStr {
    let Some(mut env) = get_env() else { return YogOwnedStr::NONE };
    let Some(jp) = ys_to_java(&mut env, player) else { return YogOwnedStr::NONE };
    let ret = env.call_static_method("dev/yog/NativeBridge", "playerInventory",
        "(Ljava/lang/String;)Ljava/lang/String;", &[JValue::Object(&jp)]);
    match ret.and_then(|v| v.l()) {
        Ok(obj) => jstring_to_owned(&mut env, obj),
        _ => YogOwnedStr::NONE,
    }
}

unsafe extern "C" fn srv_player_set_slot(_ctx: *mut c_void, player: YogStr, slot: u32, item_id: YogStr, count: u32) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jp), Some(ji)) = (ys_to_java(&mut env, player), ys_to_java(&mut env, item_id)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "playerSetSlot",
        "(Ljava/lang/String;ILjava/lang/String;I)Z",
        &[JValue::Object(&jp), JValue::Int(slot as i32), JValue::Object(&ji), JValue::Int(count as i32)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_player_teleport_dim(_ctx: *mut c_void, player: YogStr, dim: YogStr, pos: YogVec3) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jp), Some(jd)) = (ys_to_java(&mut env, player), ys_to_java(&mut env, dim)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "teleportToDim",
        "(Ljava/lang/String;Ljava/lang/String;DDD)Z",
        &[JValue::Object(&jp), JValue::Object(&jd), JValue::Double(pos.x), JValue::Double(pos.y), JValue::Double(pos.z)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_entity_teleport_dim(_ctx: *mut c_void, uuid: YogStr, dim: YogStr, pos: YogVec3) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(ju), Some(jd)) = (ys_to_java(&mut env, uuid), ys_to_java(&mut env, dim)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "entityTeleportToDim",
        "(Ljava/lang/String;Ljava/lang/String;DDD)Z",
        &[JValue::Object(&ju), JValue::Object(&jd), JValue::Double(pos.x), JValue::Double(pos.y), JValue::Double(pos.z)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_online_players(_ctx: *mut c_void) -> YogOwnedStr {
    let Some(mut env) = get_env() else { return YogOwnedStr::NONE };
    let ret = env.call_static_method("dev/yog/NativeBridge", "onlinePlayers",
        "()Ljava/lang/String;", &[]);
    match ret.and_then(|v| v.l()) {
        Ok(obj) => jstring_to_owned(&mut env, obj),
        _ => YogOwnedStr::NONE,
    }
}

unsafe extern "C" fn srv_world_entity_count(_ctx: *mut c_void, dim: YogStr, entity_type: YogStr) -> i32 {
    let Some(mut env) = get_env() else { return -1 };
    let d = dim.as_str();
    let et = entity_type.as_str();
    let jd  = match env.new_string(d)  { Ok(s) => s, Err(_) => return -1 };
    let jet = match env.new_string(et) { Ok(s) => s, Err(_) => return -1 };
    let ret = env.call_static_method("dev/yog/NativeBridge", "worldEntityCount",
        "(Ljava/lang/String;Ljava/lang/String;)I",
        &[JValue::Object(&jd), JValue::Object(&jet)]);
    match ret.and_then(|v| v.i()) {
        Ok(n) => n,
        _ => -1,
    }
}

unsafe extern "C" fn srv_game_dir(_ctx: *mut c_void) -> YogOwnedStr {
    let Some(mut env) = get_env() else { return YogOwnedStr::NONE };
    let ret = env.call_static_method("dev/yog/NativeBridge", "gameDir",
        "()Ljava/lang/String;", &[]);
    match ret.and_then(|v| v.l()) {
        Ok(obj) => jstring_to_owned(&mut env, obj),
        _ => YogOwnedStr::NONE,
    }
}

unsafe extern "C" fn srv_entity_get_nbt(_ctx: *mut c_void, uuid: YogStr) -> YogOwnedStr {
    let Some(mut env) = get_env() else { return YogOwnedStr::NONE };
    let Some(ju) = ys_to_java(&mut env, uuid) else { return YogOwnedStr::NONE };
    let ret = env.call_static_method("dev/yog/NativeBridge", "entityGetNbt",
        "(Ljava/lang/String;)Ljava/lang/String;", &[JValue::Object(&ju)]);
    match ret.and_then(|v| v.l()) {
        Ok(obj) => jstring_to_owned(&mut env, obj),
        _ => YogOwnedStr::NONE,
    }
}

unsafe extern "C" fn srv_entity_set_nbt(_ctx: *mut c_void, uuid: YogStr, snbt: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(ju), Some(js)) = (ys_to_java(&mut env, uuid), ys_to_java(&mut env, snbt)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "entitySetNbt",
        "(Ljava/lang/String;Ljava/lang/String;)Z", &[JValue::Object(&ju), JValue::Object(&js)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_spawn_particles(
    _ctx: *mut c_void, dim: YogStr, pos: YogVec3, particle_type: YogStr,
    count: i32, dx: f64, dy: f64, dz: f64, speed: f64,
) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jd), Some(jp)) = (ys_to_java(&mut env, dim), ys_to_java(&mut env, particle_type)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "spawnParticles",
        "(Ljava/lang/String;DDDLjava/lang/String;IDDDD)Z",
        &[JValue::Object(&jd), JValue::Double(pos.x), JValue::Double(pos.y), JValue::Double(pos.z),
          JValue::Object(&jp), JValue::Int(count),
          JValue::Double(dx), JValue::Double(dy), JValue::Double(dz), JValue::Double(speed)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_entity_attribute_get(_ctx: *mut c_void, uuid: YogStr, attr: YogStr) -> f64 {
    let Some(mut env) = get_env() else { return f64::NAN };
    let (Some(ju), Some(ja)) = (ys_to_java(&mut env, uuid), ys_to_java(&mut env, attr)) else { return f64::NAN };
    env.call_static_method("dev/yog/NativeBridge", "entityAttributeGet",
        "(Ljava/lang/String;Ljava/lang/String;)D", &[JValue::Object(&ju), JValue::Object(&ja)])
    .and_then(|v| v.d()).unwrap_or(f64::NAN)
}

unsafe extern "C" fn srv_entity_attribute_set(_ctx: *mut c_void, uuid: YogStr, attr: YogStr, value: f64) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(ju), Some(ja)) = (ys_to_java(&mut env, uuid), ys_to_java(&mut env, attr)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "entityAttributeSet",
        "(Ljava/lang/String;Ljava/lang/String;D)Z",
        &[JValue::Object(&ju), JValue::Object(&ja), JValue::Double(value)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_get_held_item_nbt(_ctx: *mut c_void, player: YogStr) -> YogOwnedStr {
    let Some(mut env) = get_env() else { return YogOwnedStr::NONE };
    let Some(jp) = ys_to_java(&mut env, player) else { return YogOwnedStr::NONE };
    let ret = env.call_static_method("dev/yog/NativeBridge", "getHeldItemNbt",
        "(Ljava/lang/String;)Ljava/lang/String;", &[JValue::Object(&jp)]);
    match ret.and_then(|v| v.l()) {
        Ok(obj) => jstring_to_owned(&mut env, obj),
        _ => YogOwnedStr::NONE,
    }
}

unsafe extern "C" fn srv_set_held_item_nbt(_ctx: *mut c_void, player: YogStr, snbt: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jp), Some(js)) = (ys_to_java(&mut env, player), ys_to_java(&mut env, snbt)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "setHeldItemNbt",
        "(Ljava/lang/String;Ljava/lang/String;)Z", &[JValue::Object(&jp), JValue::Object(&js)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_get_offhand_item_nbt(_ctx: *mut c_void, player: YogStr) -> YogOwnedStr {
    let Some(mut env) = get_env() else { return YogOwnedStr::NONE };
    let Some(jp) = ys_to_java(&mut env, player) else { return YogOwnedStr::NONE };
    let ret = env.call_static_method("dev/yog/NativeBridge", "getOffhandItemNbt",
        "(Ljava/lang/String;)Ljava/lang/String;", &[JValue::Object(&jp)]);
    match ret.and_then(|v| v.l()) {
        Ok(obj) => jstring_to_owned(&mut env, obj),
        _ => YogOwnedStr::NONE,
    }
}

unsafe extern "C" fn srv_set_offhand_item_nbt(_ctx: *mut c_void, player: YogStr, snbt: YogStr) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jp), Some(js)) = (ys_to_java(&mut env, player), ys_to_java(&mut env, snbt)) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "setOffhandItemNbt",
        "(Ljava/lang/String;Ljava/lang/String;)Z", &[JValue::Object(&jp), JValue::Object(&js)])
    .and_then(|v| v.z()).unwrap_or(false)
}

unsafe extern "C" fn srv_get_slot_item(_ctx: *mut c_void, player: YogStr, slot: u32) -> YogOwnedStr {
    let Some(mut env) = get_env() else { return YogOwnedStr::NONE };
    let Some(jp) = ys_to_java(&mut env, player) else { return YogOwnedStr::NONE };
    let ret = env.call_static_method("dev/yog/NativeBridge", "getSlotItem",
        "(Ljava/lang/String;I)Ljava/lang/String;",
        &[JValue::Object(&jp), JValue::Int(slot as i32)]);
    match ret.and_then(|v| v.l()) {
        Ok(obj) => jstring_to_owned(&mut env, obj),
        _ => YogOwnedStr::NONE,
    }
}

unsafe extern "C" fn srv_set_slot_item(
    _ctx: *mut c_void, player: YogStr, slot: u32,
    item_id: YogStr, count: u32, snbt: YogStr,
) -> bool {
    let Some(mut env) = get_env() else { return false };
    let (Some(jp), Some(ji), Some(js)) = (
        ys_to_java(&mut env, player), ys_to_java(&mut env, item_id), ys_to_java(&mut env, snbt)
    ) else { return false };
    env.call_static_method("dev/yog/NativeBridge", "setSlotItem",
        "(Ljava/lang/String;ILjava/lang/String;ILjava/lang/String;)Z",
        &[JValue::Object(&jp), JValue::Int(slot as i32), JValue::Object(&ji), JValue::Int(count as i32), JValue::Object(&js)])
    .and_then(|v| v.z()).unwrap_or(false)
}

// ── ABI minor 14 — low-level GPU pipeline ────────────────────────────────────
//
// All raw GL calls go through `glow::Context` stored in GL.
// `draw2d_*` functions still call JNI (NativeDraw) for MC text/texture rendering.
// Everything is called on the render thread — no synchronization needed.

// ── helpers ───────────────────────────────────────────────────────────────────

fn gl_draw_mode(mode: u8) -> u32 {
    match mode {
        1 => glow::LINES,
        2 => glow::LINE_STRIP,
        3 => glow::TRIANGLE_STRIP,
        4 => glow::TRIANGLE_FAN,
        _ => glow::TRIANGLES,
    }
}

fn gl_attr_type(dtype: u8) -> u32 {
    match dtype {
        1 => glow::UNSIGNED_BYTE,
        2 => glow::INT,
        3 => glow::UNSIGNED_INT,
        _ => glow::FLOAT,
    }
}

unsafe fn compile_shader(gl: &glow::Context, stage: u32, src: &str) -> Option<glow::NativeShader> {
    let sh = gl.create_shader(stage).ok()?;
    gl.shader_source(sh, src);
    gl.compile_shader(sh);
    if !gl.get_shader_compile_status(sh) {
        yog_logging::error!("yog-gfx shader compile: {}", gl.get_shader_info_log(sh));
        gl.delete_shader(sh);
        return None;
    }
    Some(sh)
}

// ── GPU buffers ───────────────────────────────────────────────────────────────

unsafe extern "C" fn gfx_buf_create() -> u32 {
    let Some(g) = GL.get() else { return 0 };
    g.0.create_buffer().map(|b| b.0.get()).unwrap_or(0)
}

unsafe extern "C" fn gfx_buf_delete(handle: u32) {
    let Some(g) = GL.get() else { return };
    let Some(n) = NonZeroU32::new(handle) else { return };
    g.0.delete_buffer(glow::NativeBuffer(n));
}

unsafe extern "C" fn gfx_buf_data(handle: u32, bytes: *const u8, len: u32, dynamic: bool) {
    let Some(g) = GL.get() else { return };
    let Some(n) = NonZeroU32::new(handle) else { return };
    let gl = &g.0;
    let data = std::slice::from_raw_parts(bytes, len as usize);
    let usage = if dynamic { glow::DYNAMIC_DRAW } else { glow::STATIC_DRAW };
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(glow::NativeBuffer(n)));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, data, usage);
    gl.bind_buffer(glow::ARRAY_BUFFER, None);
}

unsafe extern "C" fn gfx_buf_subdata(handle: u32, offset: u32, bytes: *const u8, len: u32) {
    let Some(g) = GL.get() else { return };
    let Some(n) = NonZeroU32::new(handle) else { return };
    let gl = &g.0;
    let data = std::slice::from_raw_parts(bytes, len as usize);
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(glow::NativeBuffer(n)));
    gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, offset as i32, data);
    gl.bind_buffer(glow::ARRAY_BUFFER, None);
}

// ── Vertex arrays ─────────────────────────────────────────────────────────────

unsafe extern "C" fn gfx_vao_create() -> u32 {
    let Some(g) = GL.get() else { return 0 };
    g.0.create_vertex_array().map(|v| v.0.get()).unwrap_or(0)
}

unsafe extern "C" fn gfx_vao_delete(handle: u32) {
    let Some(g) = GL.get() else { return };
    let Some(n) = NonZeroU32::new(handle) else { return };
    g.0.delete_vertex_array(glow::NativeVertexArray(n));
}

unsafe extern "C" fn gfx_vao_attrib(
    vao: u32, vbo: u32, index: u32, components: u8,
    dtype: u8, normalized: bool, stride: u32, offset: u32,
) {
    let Some(g) = GL.get() else { return };
    let (Some(vn), Some(bn)) = (NonZeroU32::new(vao), NonZeroU32::new(vbo)) else { return };
    let gl = &g.0;
    gl.bind_vertex_array(Some(glow::NativeVertexArray(vn)));
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(glow::NativeBuffer(bn)));
    let gl_type = gl_attr_type(dtype);
    if dtype == 2 || dtype == 3 {
        gl.vertex_attrib_pointer_i32(index, components as i32, gl_type, stride as i32, offset as i32);
    } else {
        gl.vertex_attrib_pointer_f32(index, components as i32, gl_type, normalized, stride as i32, offset as i32);
    }
    gl.enable_vertex_attrib_array(index);
    gl.bind_vertex_array(None);
}

unsafe extern "C" fn gfx_vao_set_ebo(vao: u32, ebo: u32) {
    let Some(g) = GL.get() else { return };
    let (Some(vn), Some(en)) = (NonZeroU32::new(vao), NonZeroU32::new(ebo)) else { return };
    let gl = &g.0;
    gl.bind_vertex_array(Some(glow::NativeVertexArray(vn)));
    gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(glow::NativeBuffer(en)));
    gl.bind_vertex_array(None);
}

// ── Shader programs ───────────────────────────────────────────────────────────

/// Returns a path under `~/.cache/yog/shaders/<hash>.ysc` for the given GLSL source pair,
/// creating the directory if needed.  Returns `None` if the home directory is unknown.
fn shader_cache_path(vert: &str, frag: &str) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    vert.hash(&mut h);
    frag.hash(&mut h);
    let hash = h.finish();
    let dir = std::env::var("HOME")
        .map(|home| PathBuf::from(home).join(".cache").join("yog").join("shaders"))
        .ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join(format!("{hash:016x}.ysc")))
}

/// Try restoring a program from a binary blob (`[4 LE bytes: GL format][binary…]`).
/// Returns `Some(handle)` if the driver accepts the binary, `None` otherwise.
///
/// Uses `glProgramBinary` (GL 4.1 / ARB_get_program_binary) via the raw pointer
/// captured in `nativeGlInit`.  Returns `None` if the extension is unavailable.
unsafe fn load_shader_binary(gl: &glow::Context, data: &[u8]) -> Option<glow::NativeProgram> {
    if data.len() < 4 { return None; }
    type ProgramBinaryFn = unsafe extern "system" fn(u32, u32, *const c_void, i32);
    let fn_ptr = (*GL_PROGRAM_BINARY.get()?)?;
    let program_binary: ProgramBinaryFn = std::mem::transmute(fn_ptr);

    let fmt = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let prog = gl.create_program().ok()?;
    program_binary(prog.0.get(), fmt, data[4..].as_ptr() as *const c_void, (data.len() - 4) as i32);
    if gl.get_program_link_status(prog) { Some(prog) } else { gl.delete_program(prog); None }
}

/// Read the compiled binary of a linked program.  Returns empty `Vec` if unsupported.
unsafe fn get_shader_binary(prog: glow::NativeProgram) -> (Vec<u8>, u32) {
    type GetProgramivFn       = unsafe extern "system" fn(u32, u32, *mut i32);
    type GetProgramBinaryFn   = unsafe extern "system" fn(u32, i32, *mut i32, *mut u32, *mut c_void);

    let Some(get_iv_raw)  = GL_GET_PROGRAM_IV.get().and_then(|v| *v) else { return (vec![], 0) };
    let Some(get_bin_raw) = GL_GET_PROGRAM_BINARY.get().and_then(|v| *v) else { return (vec![], 0) };
    let get_program_iv:     GetProgramivFn       = std::mem::transmute(get_iv_raw);
    let get_program_binary: GetProgramBinaryFn   = std::mem::transmute(get_bin_raw);

    // Query binary size via glGetProgramiv(GL_PROGRAM_BINARY_LENGTH).
    const PROGRAM_BINARY_LENGTH: u32 = 0x8741;
    let mut size: i32 = 0;
    get_program_iv(prog.0.get(), PROGRAM_BINARY_LENGTH, &mut size);
    if size <= 0 { return (vec![], 0); }

    let mut buf = vec![0u8; size as usize];
    let mut actual_len: i32 = 0;
    let mut fmt: u32 = 0;
    get_program_binary(prog.0.get(), size, &mut actual_len, &mut fmt, buf.as_mut_ptr() as *mut c_void);
    buf.truncate(actual_len.max(0) as usize);
    (buf, fmt)
}

unsafe extern "C" fn gfx_prog_create(vert: YogStr, frag: YogStr, out: *mut u32) -> bool {
    let Some(g) = GL.get() else { return false };
    let gl = &g.0;
    let vert_s = vert.as_str();
    let frag_s = frag.as_str();

    // Fast path: binary shader cache — avoids GLSL re-compilation on subsequent launches.
    let cache_path = shader_cache_path(vert_s, frag_s);
    if let Some(ref path) = cache_path {
        if let Ok(data) = std::fs::read(path) {
            if let Some(prog) = load_shader_binary(gl, &data) {
                *out = prog.0.get();
                return true;
            }
            // Cache stale (driver updated?); remove and fall through to GLSL compile.
            let _ = std::fs::remove_file(path);
        }
    }

    // GLSL compile path.
    let vs = match compile_shader(gl, glow::VERTEX_SHADER, vert_s) {
        Some(s) => s,
        None => return false,
    };
    let fs = match compile_shader(gl, glow::FRAGMENT_SHADER, frag_s) {
        Some(s) => s,
        None => { gl.delete_shader(vs); return false; }
    };
    let prog = match gl.create_program() {
        Ok(p) => p,
        Err(e) => {
            yog_logging::error!("yog-gfx: create_program: {}", e);
            gl.delete_shader(vs); gl.delete_shader(fs);
            return false;
        }
    };
    gl.attach_shader(prog, vs);
    gl.attach_shader(prog, fs);
    gl.link_program(prog);
    gl.detach_shader(prog, vs);
    gl.detach_shader(prog, fs);
    gl.delete_shader(vs);
    gl.delete_shader(fs);
    if !gl.get_program_link_status(prog) {
        yog_logging::error!("yog-gfx: shader link: {}", gl.get_program_info_log(prog));
        gl.delete_program(prog);
        return false;
    }

    // Persist binary so the next launch skips GLSL compilation entirely.
    if let Some(ref path) = cache_path {
        let (binary, fmt) = get_shader_binary(prog);
        if !binary.is_empty() {
            let mut blob = fmt.to_le_bytes().to_vec();
            blob.extend_from_slice(&binary);
            let _ = std::fs::write(path, &blob);
        }
    }

    *out = prog.0.get();
    true
}

unsafe extern "C" fn gfx_prog_delete(handle: u32) {
    let Some(g) = GL.get() else { return };
    let Some(n) = NonZeroU32::new(handle) else { return };
    g.0.delete_program(glow::NativeProgram(n));
}

macro_rules! with_prog {
    ($handle:expr, |$gl:ident, $prog:ident, $loc:ident ($name:expr)| $body:expr) => {{
        let Some(g) = GL.get() else { return };
        let Some(n) = NonZeroU32::new($handle) else { return };
        let $gl = &g.0;
        let $prog = glow::NativeProgram(n);
        $gl.use_program(Some($prog));
        let $loc = $gl.get_uniform_location($prog, $name.as_str());
        $body
    }};
}

unsafe extern "C" fn gfx_prog_uniform_1i(prog: u32, name: YogStr, v: i32) {
    with_prog!(prog, |gl, _p, loc(name)| gl.uniform_1_i32(loc.as_ref(), v));
}
unsafe extern "C" fn gfx_prog_uniform_1f(prog: u32, name: YogStr, v: f32) {
    with_prog!(prog, |gl, _p, loc(name)| gl.uniform_1_f32(loc.as_ref(), v));
}
unsafe extern "C" fn gfx_prog_uniform_2f(prog: u32, name: YogStr, x: f32, y: f32) {
    with_prog!(prog, |gl, _p, loc(name)| gl.uniform_2_f32(loc.as_ref(), x, y));
}
unsafe extern "C" fn gfx_prog_uniform_3f(prog: u32, name: YogStr, x: f32, y: f32, z: f32) {
    with_prog!(prog, |gl, _p, loc(name)| gl.uniform_3_f32(loc.as_ref(), x, y, z));
}
unsafe extern "C" fn gfx_prog_uniform_4f(prog: u32, name: YogStr, x: f32, y: f32, z: f32, w: f32) {
    with_prog!(prog, |gl, _p, loc(name)| gl.uniform_4_f32(loc.as_ref(), x, y, z, w));
}
unsafe extern "C" fn gfx_prog_uniform_mat4(prog: u32, name: YogStr, col_major: *const f32) {
    with_prog!(prog, |gl, _p, loc(name)| {
        let data = std::slice::from_raw_parts(col_major, 16);
        gl.uniform_matrix_4_f32_slice(loc.as_ref(), false, data);
    });
}

// ── Textures ──────────────────────────────────────────────────────────────────

unsafe extern "C" fn gfx_tex_create(w: u32, h: u32, rgba: *const u8, linear: bool) -> u32 {
    let Some(g) = GL.get() else { return 0 };
    let gl = &g.0;
    let tex = match gl.create_texture() { Ok(t) => t, Err(_) => return 0 };
    gl.bind_texture(glow::TEXTURE_2D, Some(tex));
    let filter = if linear { glow::LINEAR } else { glow::NEAREST };
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
    let pixels = std::slice::from_raw_parts(rgba, (w * h * 4) as usize);
    gl.tex_image_2d(
        glow::TEXTURE_2D, 0, glow::RGBA8 as i32,
        w as i32, h as i32, 0,
        glow::RGBA, glow::UNSIGNED_BYTE,
        Some(pixels),
    );
    gl.bind_texture(glow::TEXTURE_2D, None);
    tex.0.get()
}

unsafe extern "C" fn gfx_tex_delete(handle: u32) {
    let Some(g) = GL.get() else { return };
    let Some(n) = NonZeroU32::new(handle) else { return };
    g.0.delete_texture(glow::NativeTexture(n));
}

unsafe extern "C" fn gfx_tex_bind(unit: u32, handle: u32) {
    let Some(g) = GL.get() else { return };
    let gl = &g.0;
    gl.active_texture(glow::TEXTURE0 + unit);
    match NonZeroU32::new(handle) {
        Some(n) => gl.bind_texture(glow::TEXTURE_2D, Some(glow::NativeTexture(n))),
        None    => gl.bind_texture(glow::TEXTURE_2D, None),
    }
}

unsafe extern "C" fn gfx_tex_from_mc(id: YogStr) -> u32 {
    let Some(mut env) = get_env() else { return 0 };
    let Some(ji) = ys_to_java(&mut env, id) else { return 0 };
    env.call_static_method("dev/yog/NativeDraw", "getMcTextureId",
        "(Ljava/lang/String;)I", &[JValue::Object(&ji)])
        .and_then(|v| v.i())
        .map(|id| id as u32)
        .unwrap_or(0)
}

// ── Draw calls ────────────────────────────────────────────────────────────────

unsafe extern "C" fn gfx_draw_arrays(vao: u32, prog: u32, mode: u8, first: u32, count: u32) {
    let Some(g) = GL.get() else { return };
    let (Some(vn), Some(pn)) = (NonZeroU32::new(vao), NonZeroU32::new(prog)) else { return };
    let gl = &g.0;
    gl.use_program(Some(glow::NativeProgram(pn)));
    gl.bind_vertex_array(Some(glow::NativeVertexArray(vn)));
    gl.draw_arrays(gl_draw_mode(mode), first as i32, count as i32);
    gl.bind_vertex_array(None);
}

unsafe extern "C" fn gfx_draw_elements(vao: u32, ebo: u32, prog: u32, mode: u8, count: u32, u32_idx: bool) {
    let Some(g) = GL.get() else { return };
    let (Some(vn), Some(pn)) = (NonZeroU32::new(vao), NonZeroU32::new(prog)) else { return };
    let gl = &g.0;
    gl.use_program(Some(glow::NativeProgram(pn)));
    gl.bind_vertex_array(Some(glow::NativeVertexArray(vn)));
    // EBO is stored in the VAO; ebo param is informational for safety but not re-bound here.
    let _ = ebo;
    let idx_type = if u32_idx { glow::UNSIGNED_INT } else { glow::UNSIGNED_SHORT };
    gl.draw_elements(gl_draw_mode(mode), count as i32, idx_type, 0);
    gl.bind_vertex_array(None);
}

// ── Render state ──────────────────────────────────────────────────────────────

unsafe extern "C" fn gfx_set_blend(enabled: bool, src: u32, dst: u32) {
    let Some(g) = GL.get() else { return };
    let gl = &g.0;
    if enabled {
        gl.enable(glow::BLEND);
        gl.blend_func(src, dst);
    } else {
        gl.disable(glow::BLEND);
    }
}

unsafe extern "C" fn gfx_set_depth(test: bool, write: bool) {
    let Some(g) = GL.get() else { return };
    let gl = &g.0;
    if test { gl.enable(glow::DEPTH_TEST); } else { gl.disable(glow::DEPTH_TEST); }
    gl.depth_mask(write);
}

unsafe extern "C" fn gfx_set_scissor(x: i32, y: i32, w: i32, h: i32) {
    let Some(g) = GL.get() else { return };
    let gl = &g.0;
    gl.enable(glow::SCISSOR_TEST);
    gl.scissor(x, y, w, h);
}

unsafe extern "C" fn gfx_clear_scissor() {
    let Some(g) = GL.get() else { return };
    g.0.disable(glow::SCISSOR_TEST);
}

unsafe extern "C" fn gfx_set_viewport(x: i32, y: i32, w: i32, h: i32) {
    let Some(g) = GL.get() else { return };
    g.0.viewport(x, y, w, h);
}

// ── 2D convenience (JNI — uses MC's DrawContext / text renderer) ──────────────

unsafe extern "C" fn gfx_draw2d_rect(x1: f32, y1: f32, x2: f32, y2: f32, color: u32) {
    let Some(mut env) = get_env() else { return };
    let _ = env.call_static_method("dev/yog/NativeDraw", "drawRect",
        "(FFFFI)V",
        &[JValue::Float(x1), JValue::Float(y1), JValue::Float(x2), JValue::Float(y2),
          JValue::Int(color as i32)]);
}

unsafe extern "C" fn gfx_draw2d_gradient(x1: f32, y1: f32, x2: f32, y2: f32, top: u32, bottom: u32) {
    let Some(mut env) = get_env() else { return };
    let _ = env.call_static_method("dev/yog/NativeDraw", "drawGradientRect",
        "(FFFFII)V",
        &[JValue::Float(x1), JValue::Float(y1), JValue::Float(x2), JValue::Float(y2),
          JValue::Int(top as i32), JValue::Int(bottom as i32)]);
}

unsafe extern "C" fn gfx_draw2d_text(text: YogStr, x: f32, y: f32, color: u32, shadow: bool) {
    let Some(mut env) = get_env() else { return };
    if let Some(jt) = ys_to_java(&mut env, text) {
        let _ = env.call_static_method("dev/yog/NativeDraw", "drawText",
            "(Ljava/lang/String;FFIZ)V",
            &[JValue::Object(&jt), JValue::Float(x), JValue::Float(y),
              JValue::Int(color as i32), JValue::Bool(shadow as u8)]);
    }
}

unsafe extern "C" fn gfx_draw2d_mc_tex(
    id: YogStr, x: f32, y: f32, u0: f32, v0: f32, w: f32, h: f32, tw: f32, th: f32,
) {
    let Some(mut env) = get_env() else { return };
    if let Some(ji) = ys_to_java(&mut env, id) {
        let _ = env.call_static_method("dev/yog/NativeDraw", "drawTexture",
            "(Ljava/lang/String;FFFFFFFFF)V",
            &[JValue::Object(&ji),
              JValue::Float(x), JValue::Float(y), JValue::Float(u0), JValue::Float(v0),
              JValue::Float(w), JValue::Float(h), JValue::Float(tw), JValue::Float(th)]);
    }
}

// ── Static GFX function table (function pointers only — per-frame data filled at call time) ──

static GFX_FN_TABLE: YogGfxApi = YogGfxApi {
    // Per-frame fields zeroed in the static; actual values are set on the stack per render call.
    screen_w: 0, screen_h: 0, delta_tick: 0.0, scale_factor: 1.0,
    view_proj: [0.0; 16], camera_pos: [0.0; 3], player_pos: [0.0; 3], _pad1: 0.0,
    buf_create:        gfx_buf_create,
    buf_delete:        gfx_buf_delete,
    buf_data:          gfx_buf_data,
    buf_subdata:       gfx_buf_subdata,
    vao_create:        gfx_vao_create,
    vao_delete:        gfx_vao_delete,
    vao_attrib:        gfx_vao_attrib,
    vao_set_ebo:       gfx_vao_set_ebo,
    prog_create:       gfx_prog_create,
    prog_delete:       gfx_prog_delete,
    prog_uniform_1i:   gfx_prog_uniform_1i,
    prog_uniform_1f:   gfx_prog_uniform_1f,
    prog_uniform_2f:   gfx_prog_uniform_2f,
    prog_uniform_3f:   gfx_prog_uniform_3f,
    prog_uniform_4f:   gfx_prog_uniform_4f,
    prog_uniform_mat4: gfx_prog_uniform_mat4,
    tex_create:        gfx_tex_create,
    tex_delete:        gfx_tex_delete,
    tex_bind:          gfx_tex_bind,
    tex_from_mc:       gfx_tex_from_mc,
    draw_arrays:       gfx_draw_arrays,
    draw_elements:     gfx_draw_elements,
    set_blend:         gfx_set_blend,
    set_depth:         gfx_set_depth,
    set_scissor:       gfx_set_scissor,
    clear_scissor:     gfx_clear_scissor,
    set_viewport:      gfx_set_viewport,
    draw2d_rect:       gfx_draw2d_rect,
    draw2d_gradient:   gfx_draw2d_gradient,
    draw2d_text:       gfx_draw2d_text,
    draw2d_mc_tex:     gfx_draw2d_mc_tex,
};

// ── YogApi registration functions ─────────────────────────────────────────────
//
// ctx is *mut RuntimeHandlers (cast from *mut c_void).

macro_rules! api_event {
    ($name:ident, $field:ident, $fn_ty:ty) => {
        unsafe extern "C" fn $name(ctx: *mut c_void, ud: *mut c_void, h: $fn_ty) {
            let handlers = &mut *(ctx as *mut RuntimeHandlers);
            handlers.$field.push((ud, h));
        }
    };
}

api_event!(api_on_block_break,      block_break,        YogBlockBreakFn);
api_event!(api_on_chat,             chat,               YogChatFn);
api_event!(api_on_player_join,      player_join,        YogPlayerFn);
api_event!(api_on_player_leave,     player_leave,       YogPlayerFn);
api_event!(api_on_use_item,         use_item,           YogUseItemFn);
api_event!(api_on_use_block,        use_block,          YogUseBlockFn);
api_event!(api_on_attack_entity,    attack_entity,      YogAttackEntityFn);
api_event!(api_on_entity_damage,    entity_damage,      YogEntityDamageFn);
api_event!(api_on_entity_death,     entity_death,       YogEntityDeathFn);
api_event!(api_on_entity_spawn,     entity_spawn,       YogEntitySpawnFn);
api_event!(api_on_player_place_block, player_place_block, YogPlaceBlockFn);
api_event!(api_on_player_death,     player_death,       YogPlayerDeathFn);
api_event!(api_on_player_respawn,   player_respawn,     YogPlayerRespawnFn);
api_event!(api_on_advancement,      advancement,        YogAdvancementFn);
api_event!(api_on_entity_interact,  entity_interact,    YogEntityInteractFn);
api_event!(api_on_item_craft,       item_craft,         YogCraftFn);
api_event!(api_on_explosion,        explosion,          YogExplosionFn);
api_event!(api_on_item_pickup,      item_pickup,        YogItemPickupFn);
api_event!(api_on_player_move,      player_move,        YogPlayerMoveFn);
api_event!(api_on_container_open,   container_open,     YogContainerOpenFn);
api_event!(api_on_container_close,  container_close,    YogContainerCloseFn);
api_event!(api_on_projectile_hit,   projectile_hit,     YogProjectileHitFn);
api_event!(api_on_client_tick,      client_tick,        YogClientFn);
api_event!(api_on_hud_render,       hud_render,         YogHudRenderFn);
api_event!(api_on_world_render,     world_render,       YogWorldRenderFn);
api_event!(api_on_key_press,        key_press,          YogKeyPressFn);
api_event!(api_on_screen_open,      screen_open,        YogScreenFn);
api_event!(api_on_screen_close,     screen_close,       YogScreenFn);
api_event!(api_on_server_tick,      server_tick,        YogServerFn);
api_event!(api_on_server_started,   server_started,     YogServerFn);
api_event!(api_on_server_stopping,  server_stopping,    YogServerFn);

unsafe extern "C" fn api_on_packet(ctx: *mut c_void, channel: YogStr, ud: *mut c_void, h: YogPacketFn) {
    let handlers = &mut *(ctx as *mut RuntimeHandlers);
    handlers.packets.insert(channel.as_str().to_owned(), (ud, h));
}

unsafe extern "C" fn api_on_client_packet(ctx: *mut c_void, channel: YogStr, ud: *mut c_void, h: YogPacketFn) {
    let handlers = &mut *(ctx as *mut RuntimeHandlers);
    handlers.client_packets.insert(channel.as_str().to_owned(), (ud, h));
}

unsafe extern "C" fn api_register_command(ctx: *mut c_void, name: YogStr, ud: *mut c_void, h: YogCommandFn) {
    let handlers = &mut *(ctx as *mut RuntimeHandlers);
    handlers.commands.insert(name.as_str().to_owned(), (ud, h));
}

unsafe extern "C" fn api_register_typed_command(ctx: *mut c_void, name: YogStr, schema: YogStr, ud: *mut c_void, h: YogCommandFn) {
    let handlers = &mut *(ctx as *mut RuntimeHandlers);
    let n = name.as_str().to_owned();
    handlers.typed_schemas.insert(n.clone(), schema.as_str().to_owned());
    handlers.commands.insert(n, (ud, h));
}


unsafe extern "C" fn api_register_recipe_json(ctx: *mut c_void, namespace: YogStr, name: YogStr, json: YogStr) {
    let handlers = &mut *(ctx as *mut RuntimeHandlers);
    handlers.recipes.push((
        namespace.as_str().to_owned(),
        name.as_str().to_owned(),
        json.as_str().to_owned(),
    ));
}

unsafe extern "C" fn api_register_item(ctx: *mut c_void, def: *const YogItemDef) {
    let handlers = &mut *(ctx as *mut RuntimeHandlers);
    let d = &*def;
    let food = if d.food_nutrition > 0 {
        Some(FoodDef { nutrition: d.food_nutrition, saturation: d.food_saturation, can_always_eat: d.food_always_eat })
    } else { None };
    handlers.items.push(ItemDef {
        id:            d.id.as_str().to_owned(),
        max_stack:     d.max_stack as u8,
        name:          if d.name.is_empty() { None } else { Some(d.name.as_str().to_owned()) },
        tooltip:       if d.tooltip.is_empty() { None } else { Some(d.tooltip.as_str().to_owned()) },
        max_damage:    d.max_damage,
        fire_resistant: d.fire_resistant,
        fuel_ticks:    d.fuel_ticks,
        food,
    });
}

unsafe extern "C" fn api_register_block(ctx: *mut c_void, def: *const YogBlockDef) {
    let handlers = &mut *(ctx as *mut RuntimeHandlers);
    let d = &*def;
    handlers.blocks.push(BlockDef {
        id:            d.id.as_str().to_owned(),
        hardness:      d.hardness,
        resistance:    d.resistance,
        name:          if d.name.is_empty() { None } else { Some(d.name.as_str().to_owned()) },
        light_level:   d.light_level,
        sound:         if d.sound.is_empty() { None } else { Some(d.sound.as_str().to_owned()) },
        requires_tool: d.requires_tool,
        no_collision:  d.no_collision,
        slipperiness:  d.slipperiness,
        shape:         if d.shape == [0.0f32; 6] { None } else { Some(d.shape) },
    });
}

unsafe extern "C" fn api_schedule_once(ctx: *mut c_void, delay_ticks: u64, ud: *mut c_void, h: YogScheduledFn) {
    let handlers = &mut *(ctx as *mut RuntimeHandlers);
    handlers.scheduler.lock().expect("scheduler poisoned").once_tasks.push(OnceTask { delay_remaining: delay_ticks, ud, f: h });
}

unsafe extern "C" fn api_schedule_repeating(ctx: *mut c_void, period_ticks: u64, ud: *mut c_void, h: YogScheduledFn) {
    let handlers = &mut *(ctx as *mut RuntimeHandlers);
    handlers.scheduler.lock().expect("scheduler poisoned").repeating_tasks.push(RepeatingTask { period: period_ticks, ticks_left: period_ticks, ud, f: h });
}

unsafe extern "C" fn api_register_startup_grant(ctx: *mut c_void, grant: *const YogStartupGrantDef) {
    let handlers = &mut *(ctx as *mut RuntimeHandlers);
    let g = &*grant;
    let items: Vec<String> = if g.items.is_empty() {
        Vec::new()
    } else {
        unsafe { g.items.as_str().split('|').map(|s: &str| s.to_owned()).collect() }
    };
    let book = if g.book.is_empty() { None } else { Some(unsafe { g.book.as_str().to_owned() }) };
    let command = if g.command.is_empty() { None } else { Some(unsafe { g.command.as_str().to_owned() }) };
    handlers.startup_grants.push(yog_registry::StartupGrant {
        id: unsafe { g.id.as_str().to_owned() },
        items,
        book,
        command,
    });
}

unsafe extern "C" fn api_register_book(ctx: *mut c_void, book_id: YogStr, book_json: YogStr) {
    let handlers = &mut *(ctx as *mut RuntimeHandlers);
    handlers.books.insert(unsafe { book_id.as_str().to_owned() }, unsafe { book_json.as_str().to_owned() });
}

unsafe extern "C" fn api_register_ui(ctx: *mut c_void, ui_id: YogStr, _layout_json: YogStr,
                                     ud: *mut c_void, h: yog_abi::YogUIEventFn) {
    let handlers = &mut *(ctx as *mut RuntimeHandlers);
    let id = unsafe { ui_id.as_str().to_owned() };
    handlers.ui_handlers.insert(id.clone(), (ud, h));
    yog_logging::info!("registered UI handler: {}", id);
}


#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeBookJson<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>, book_id: JString<'l>,
) -> jstring {
    let id = match env.get_string(&book_id) {
        Ok(s) => String::from(s),
        Err(_) => return std::ptr::null_mut(),
    };
    let json = handlers().books.get(&id).cloned().unwrap_or_else(|| "null".to_string());
    env.new_string(json).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

// ── Table constructors ────────────────────────────────────────────────────────

fn build_server_table() -> YogServer {
    YogServer {
        ctx:         std::ptr::null_mut(),
        abi_version: ABI_VERSION,
        size:        std::mem::size_of::<YogServer>() as u32,
        free_str:    yog_free_str,
        broadcast:   srv_broadcast,
        get_block:   srv_get_block,
        set_block:   srv_set_block,
        world_time:  srv_world_time,
        set_time:    srv_set_time,
        is_raining:  srv_is_raining,
        set_weather: srv_set_weather,
        give_item:   srv_give_item,
        player_teleport: srv_player_teleport,
        send_to_player: srv_send_to_player,
        send_to_server: srv_send_to_server,
        kick_player: srv_kick_player,
        set_gamemode: srv_set_gamemode,
        send_title:  srv_send_title,
        send_actionbar: srv_send_actionbar,
        play_sound:  srv_play_sound,
        play_sound_player: srv_play_sound_player,
        entity_teleport: srv_entity_teleport,
        entity_position: srv_entity_position,
        entity_health: srv_entity_health,
        entity_set_health: srv_entity_set_health,
        entity_kill: srv_entity_kill,
        spawn_entity: srv_spawn_entity,
        entity_add_effect: srv_entity_add_effect,
        entity_remove_effect: srv_entity_remove_effect,
        entity_clear_effects: srv_entity_clear_effects,
        entity_velocity: srv_entity_velocity,
        entity_set_velocity: srv_entity_set_velocity,
        entity_add_velocity: srv_entity_add_velocity,
        has_item_tag: srv_has_item_tag,
        has_block_tag: srv_has_block_tag,
        drop_loot: srv_drop_loot,
        scoreboard_get: srv_scoreboard_get,
        scoreboard_set: srv_scoreboard_set,
        scoreboard_add: srv_scoreboard_add,
        bossbar_create: srv_bossbar_create,
        bossbar_remove: srv_bossbar_remove,
        bossbar_set_title: srv_bossbar_set_title,
        bossbar_set_progress: srv_bossbar_set_progress,
        bossbar_set_color: srv_bossbar_set_color,
        bossbar_add_player: srv_bossbar_add_player,
        bossbar_remove_player: srv_bossbar_remove_player,
        bossbar_set_visible: srv_bossbar_set_visible,
        game_dir: srv_game_dir,
        get_block_nbt:       srv_get_block_nbt,
        set_block_nbt:       srv_set_block_nbt,
        player_inventory:    srv_player_inventory,
        player_set_slot:     srv_player_set_slot,
        player_teleport_dim: srv_player_teleport_dim,
        entity_teleport_dim: srv_entity_teleport_dim,
        online_players:      srv_online_players,
        world_entity_count:  srv_world_entity_count,
        entity_get_nbt:          srv_entity_get_nbt,
        entity_set_nbt:          srv_entity_set_nbt,
        spawn_particles:         srv_spawn_particles,
        entity_attribute_get:    srv_entity_attribute_get,
        entity_attribute_set:    srv_entity_attribute_set,
        get_held_item_nbt:       srv_get_held_item_nbt,
        set_held_item_nbt:       srv_set_held_item_nbt,
        get_offhand_item_nbt:    srv_get_offhand_item_nbt,
        set_offhand_item_nbt:    srv_set_offhand_item_nbt,
        get_slot_item:           srv_get_slot_item,
        set_slot_item:           srv_set_slot_item,
    }
}

fn build_api_table(ctx: *mut RuntimeHandlers, server: *const YogServer) -> YogApi {
    YogApi {
        abi_version: ABI_VERSION,
        size:        std::mem::size_of::<YogApi>() as u32,
        ctx:         ctx as *mut c_void,
        server,
        on_block_break:     api_on_block_break,
        on_chat:            api_on_chat,
        on_player_join:     api_on_player_join,
        on_player_leave:    api_on_player_leave,
        on_use_item:        api_on_use_item,
        on_use_block:       api_on_use_block,
        on_attack_entity:   api_on_attack_entity,
        on_entity_damage:   api_on_entity_damage,
        on_entity_death:    api_on_entity_death,
        on_entity_spawn:         api_on_entity_spawn,
        on_player_place_block:   api_on_player_place_block,
        on_player_death:         api_on_player_death,
        on_player_respawn:       api_on_player_respawn,
        on_advancement:          api_on_advancement,
        on_entity_interact:      api_on_entity_interact,
        on_item_craft:           api_on_item_craft,
        on_explosion:            api_on_explosion,
        on_item_pickup:          api_on_item_pickup,
        on_player_move:          api_on_player_move,
        on_container_open:       api_on_container_open,
        on_container_close:      api_on_container_close,
        on_projectile_hit:       api_on_projectile_hit,
        on_server_tick:          api_on_server_tick,
        on_server_started:       api_on_server_started,
        on_server_stopping:      api_on_server_stopping,
        on_packet:               api_on_packet,
        on_client_packet:        api_on_client_packet,
        register_command:        api_register_command,
        register_typed_command:  api_register_typed_command,
        register_recipe_json:    api_register_recipe_json,
        register_item:          api_register_item,
        register_block:     api_register_block,
        schedule_once:      api_schedule_once,
        schedule_repeating: api_schedule_repeating,
        on_client_tick:     api_on_client_tick,
        on_hud_render:      api_on_hud_render,
        on_key_press:       api_on_key_press,
        on_screen_open:     api_on_screen_open,
        on_screen_close:    api_on_screen_close,
        on_world_render:    api_on_world_render,
        register_startup_grant: api_register_startup_grant,
        register_book:          api_register_book,
        register_ui:            api_register_ui,
    }
}

// ── Mod loading ───────────────────────────────────────────────────────────────

fn platform_tag() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

type AbiVersionFn   = unsafe extern "C" fn() -> u32;
type RegisterFn     = unsafe extern "C" fn(*const YogApi, *mut c_void);

fn load_mods(dir: &Path, api: &YogApi) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => {
            yog_logging::info!("no mods directory at {} — none loaded", dir.display());
            return;
        }
    };
    let mut count = 0u32;
    for entry in entries.flatten() {
        let path = entry.path();
        let lib_path = match path.extension().and_then(|e| e.to_str()) {
            Some("yog") => match extract_yog(&path) {
                Some(p) => p,
                None => {
                    yog_logging::error!("no native for {} in {}", platform_tag(), path.display());
                    continue;
                }
            },
            Some("so") | Some("dll") | Some("dylib") => path.clone(),
            _ => continue,
        };
        if load_mod_lib(&lib_path, api) { count += 1; }
    }
    yog_logging::info!("loaded {} mod(s) from {}", count, dir.display());
}

fn load_mod_lib(path: &Path, api: &YogApi) -> bool {
    unsafe {
        let lib = match Library::new(path) {
            Ok(l) => l,
            Err(e) => { yog_logging::error!("failed to load {}: {}", path.display(), e); return false; }
        };
        let abi: Symbol<AbiVersionFn> = match lib.get(b"yog_abi_version") {
            Ok(s) => s,
            Err(_) => { yog_logging::error!("{} is not a Yog mod (no yog_abi_version)", path.display()); return false; }
        };
        let mod_abi = abi();
        if mod_abi != ABI_VERSION {
            yog_logging::error!("{}: ABI {} incompatible with runtime ABI {}", path.display(), mod_abi, ABI_VERSION);
            return false;
        }
        let register: Symbol<RegisterFn> = match lib.get(b"yog_mod_register") {
            Ok(s) => s,
            Err(_) => { yog_logging::error!("{} missing yog_mod_register", path.display()); return false; }
        };
        register(api as *const YogApi, std::ptr::null_mut());
        drop(register);
        drop(abi);
        LOADED_MODS.lock().expect("mods lock poisoned").push(lib);
    }
    true
}

fn extract_yog(path: &Path) -> Option<PathBuf> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let prefix = format!("natives/{}/", platform_tag());
    let mut entry_name = None;
    for i in 0..archive.len() {
        let f = archive.by_index(i).ok()?;
        if f.name().starts_with(&prefix) && !f.name().ends_with('/') {
            entry_name = Some(f.name().to_string());
            break;
        }
    }
    let entry_name = entry_name?;
    let ext = Path::new(&entry_name).extension().and_then(|e| e.to_str()).unwrap_or("bin");
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let out = std::env::temp_dir().join(format!("yog-{}-{}.{}", stem, std::process::id(), ext));
    let mut entry = archive.by_name(&entry_name).ok()?;
    let mut out_file = std::fs::File::create(&out).ok()?;
    std::io::copy(&mut entry, &mut out_file).ok()?;
    Some(out)
}

// ── Dispatcher helpers ────────────────────────────────────────────────────────

fn srv_ptr() -> *const YogServer {
    SERVER.get().expect("yog: SERVER not initialised") as *const YogServer
}

// ── JNI entry points ──────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeInit<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    mods_dir: JString<'l>,
) {
    if let Ok(vm) = env.get_java_vm() { let _ = JAVA_VM.set(vm); }

    let dir = env.get_string(&mods_dir).map(String::from).unwrap_or_default();

    // Build YogServer and store in static (gets a stable address).
    let _ = SERVER.set(build_server_table());
    let server_ptr = SERVER.get().unwrap() as *const YogServer;

    // Build RuntimeHandlers on the heap temporarily so we have a stable pointer
    // to pass as ctx while mods register.
    let mut handlers = Box::new(RuntimeHandlers::new());
    let handlers_ptr = &mut *handlers as *mut RuntimeHandlers;

    let api = build_api_table(handlers_ptr, server_ptr);

    guard("mod loading", || {
        load_mods(Path::new(&dir), &api);
    });

    // Move handlers out of Box and into the OnceLock.
    let _ = HANDLERS.set(*handlers);

    yog_logging::info!("runtime initialised — the gate is open.");
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnBlockBreak<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, block: JString<'l>, x: jint, y: jint, z: jint,
) {
    let (p, b) = (jstr!(env, player), jstr!(env, block));
    let ev = yog_abi::YogBlockBreakEvent {
        player: YogStr::from_str(&p), block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
    };
    let srv = srv_ptr();
    guard("on_block_break", || {
        for (ud, f) in &handlers().block_break {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnChat<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, message: JString<'l>,
) {
    let (p, m) = (jstr!(env, player), jstr!(env, message));
    let ev = yog_abi::YogChatEvent { player: YogStr::from_str(&p), message: YogStr::from_str(&m) };
    let srv = srv_ptr();
    guard("on_chat", || {
        for (ud, f) in &handlers().chat {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerJoin<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, uuid: JString<'l>,
) {
    let (p, u) = (jstr!(env, player), jstr!(env, uuid));
    let ev = yog_abi::YogPlayerEvent { player: YogStr::from_str(&p), uuid: YogStr::from_str(&u) };
    let srv = srv_ptr();
    guard("on_player_join", || {
        for (ud, f) in &handlers().player_join {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
    // Process startup grants (give items/books on first join).
    let h = handlers();
    let mut granted = h.startup_granted.lock().expect("startup_granted poisoned");
    yog_logging::info!("processing {} startup grants for player {}", h.startup_grants.len(), p);
    for sg in &h.startup_grants {
        let key = format!("{}::{}", u, sg.id);
        if granted.contains_key(&key) {
            yog_logging::info!("startup grant {} already granted, skipping", sg.id);
            continue;
        }
        yog_logging::info!("granting startup grant {} with {} items", sg.id, sg.items.len());
        for item_id in &sg.items {
            let ok = unsafe { srv_give_item(std::ptr::null_mut(), YogStr::from_str(&p), YogStr::from_str(item_id), 1) };
            yog_logging::info!("gave {} to {} -> {}", item_id, p, ok);
        }
        if let Some(book) = &sg.book {
            let ok = unsafe { srv_give_item(std::ptr::null_mut(), YogStr::from_str(&p), YogStr::from_str("minecraft:written_book"), 1) };
            yog_logging::info!("gave book {} to {} -> {}", book, p, ok);
        }
        granted.insert(key, true);
    }
    drop(granted);
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerLeave<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, uuid: JString<'l>,
) {
    let (p, u) = (jstr!(env, player), jstr!(env, uuid));
    let ev = yog_abi::YogPlayerEvent { player: YogStr::from_str(&p), uuid: YogStr::from_str(&u) };
    let srv = srv_ptr();
    guard("on_player_leave", || {
        for (ud, f) in &handlers().player_leave {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnUseItem<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, item: JString<'l>,
) {
    let (p, i) = (jstr!(env, player), jstr!(env, item));
    let ev = yog_abi::YogUseItemEvent { player: YogStr::from_str(&p), item: YogStr::from_str(&i) };
    let srv = srv_ptr();
    guard("on_use_item", || {
        for (ud, f) in &handlers().use_item {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnUseBlock<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, block: JString<'l>, x: jint, y: jint, z: jint,
) {
    let (p, b) = (jstr!(env, player), jstr!(env, block));
    let ev = yog_abi::YogUseBlockEvent {
        player: YogStr::from_str(&p), block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
    };
    let srv = srv_ptr();
    guard("on_use_block", || {
        for (ud, f) in &handlers().use_block {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnAttackEntity<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, target_type: JString<'l>, target_uuid: JString<'l>,
) {
    let (p, tt, tu) = (jstr!(env, player), jstr!(env, target_type), jstr!(env, target_uuid));
    let ev = yog_abi::YogAttackEntityEvent {
        player: YogStr::from_str(&p), target_type: YogStr::from_str(&tt), target_uuid: YogStr::from_str(&tu),
    };
    let srv = srv_ptr();
    guard("on_attack_entity", || {
        for (ud, f) in &handlers().attack_entity {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityDamage<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    entity_type: JString<'l>, uuid: JString<'l>, amount: jfloat, source: JString<'l>,
) {
    let (et, u, s) = (jstr!(env, entity_type), jstr!(env, uuid), jstr!(env, source));
    let ev = yog_abi::YogEntityDamageEvent {
        entity_type: YogStr::from_str(&et), uuid: YogStr::from_str(&u),
        amount, source: YogStr::from_str(&s),
    };
    let srv = srv_ptr();
    guard("on_entity_damage", || {
        for (ud, f) in &handlers().entity_damage {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityDeath<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    entity_type: JString<'l>, uuid: JString<'l>, source: JString<'l>,
) {
    let (et, u, s) = (jstr!(env, entity_type), jstr!(env, uuid), jstr!(env, source));
    let ev = yog_abi::YogEntityDeathEvent {
        entity_type: YogStr::from_str(&et), uuid: YogStr::from_str(&u), source: YogStr::from_str(&s),
    };
    let srv = srv_ptr();
    guard("on_entity_death", || {
        for (ud, f) in &handlers().entity_death {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnTick<'l>(
    _env: JNIEnv<'l>, _class: JClass<'l>,
) {
    let h = handlers();
    let srv = srv_ptr();
    guard("on_tick", || {
        for (ud, f) in &h.server_tick {
            unsafe { f(*ud, srv) };
        }
    });

    // Scheduler — once tasks
    {
        let mut sched = h.scheduler.lock().expect("scheduler poisoned");
        let mut to_fire: Vec<(*mut c_void, YogScheduledFn)> = Vec::new();
        let mut remaining = Vec::new();
        for task in sched.once_tasks.drain(..) {
            if task.delay_remaining == 0 {
                to_fire.push((task.ud, task.f));
            } else {
                remaining.push(OnceTask { delay_remaining: task.delay_remaining - 1, ..task });
            }
        }
        sched.once_tasks = remaining;
        drop(sched);
        for (ud, f) in to_fire {
            guard("schedule_once", || unsafe { f(ud, srv) });
        }
    }

    // Scheduler — repeating tasks
    {
        let mut sched = h.scheduler.lock().expect("scheduler poisoned");
        let mut to_fire: Vec<(*mut c_void, YogScheduledFn)> = Vec::new();
        for task in &mut sched.repeating_tasks {
            if task.ticks_left == 0 {
                to_fire.push((task.ud, task.f));
                task.ticks_left = task.period;
            } else {
                task.ticks_left -= 1;
            }
        }
        drop(sched);
        for (ud, f) in to_fire {
            guard("schedule_repeating", || unsafe { f(ud, srv) });
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnServerStarted<'l>(
    _env: JNIEnv<'l>, _class: JClass<'l>,
) {
    let srv = srv_ptr();
    guard("on_server_started", || {
        for (ud, f) in &handlers().server_started {
            unsafe { f(*ud, srv) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnServerStopping<'l>(
    _env: JNIEnv<'l>, _class: JClass<'l>,
) {
    let srv = srv_ptr();
    guard("on_server_stopping", || {
        for (ud, f) in &handlers().server_stopping {
            unsafe { f(*ud, srv) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeCommandNames<'l>(
    env: JNIEnv<'l>, _class: JClass<'l>,
) -> jstring {
    let names = handlers().commands.keys().cloned().collect::<Vec<_>>().join("\n");
    env.new_string(names).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnBlockBreakPre<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, block: JString<'l>, x: jint, y: jint, z: jint,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.block_break.is_empty() { return 1; }
    let p = match env.get_string(&player) { Ok(s) => String::from(s), Err(_) => return 1 };
    let b = match env.get_string(&block)  { Ok(s) => String::from(s), Err(_) => return 1 };
    let ev = yog_abi::YogBlockBreakEvent {
        player: YogStr::from_str(&p), block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
    };
    let srv = srv_ptr();
    let mut allow = true;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for (ud, f) in &h.block_break {
            if !unsafe { f(*ud, srv, &ev, 0) } { allow = false; break; }
        }
    })).ok();
    allow as jni::sys::jboolean
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnChatPre<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, message: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.chat.is_empty() { return 1; }
    let p = match env.get_string(&player)  { Ok(s) => String::from(s), Err(_) => return 1 };
    let m = match env.get_string(&message) { Ok(s) => String::from(s), Err(_) => return 1 };
    let ev = yog_abi::YogChatEvent { player: YogStr::from_str(&p), message: YogStr::from_str(&m) };
    let srv = srv_ptr();
    let mut allow = true;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for (ud, f) in &h.chat {
            if !unsafe { f(*ud, srv, &ev, 0) } { allow = false; break; }
        }
    })).ok();
    allow as jni::sys::jboolean
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeRecipeJsons<'l>(
    env: JNIEnv<'l>, _class: JClass<'l>,
) -> jstring {
    let s = handlers().recipes.iter()
        .map(|(ns, name, json)| format!("{}\t{}\t{}", ns, name, json))
        .collect::<Vec<_>>().join("\n");
    env.new_string(s).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeTypedCommandSchemas<'l>(
    env: JNIEnv<'l>, _class: JClass<'l>,
) -> jstring {
    let s = handlers().typed_schemas.iter()
        .map(|(name, schema)| format!("{}\t{}", name, schema))
        .collect::<Vec<_>>().join("\n");
    env.new_string(s).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnCommand<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    name: JString<'l>, args: JString<'l>, source: JString<'l>, uuid: JString<'l>,
) -> jstring {
    let (n, a, s, u) = (
        env.get_string(&name).map(String::from).unwrap_or_default(),
        env.get_string(&args).map(String::from).unwrap_or_default(),
        env.get_string(&source).map(String::from).unwrap_or_default(),
        env.get_string(&uuid).map(String::from).unwrap_or_default(),
    );
    let ev = yog_abi::YogCommandEvent {
        name: YogStr::from_str(&n), args: YogStr::from_str(&a),
        source: YogStr::from_str(&s), uuid: YogStr::from_str(&u),
    };
    let h = handlers();
    let srv = srv_ptr();
    let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Some((ud, f)) = h.commands.get(&n) {
            let mut buf = [0u8; 4096];
            let mut reply_len: u32 = 0;
            unsafe { f(*ud, srv, &ev, buf.as_mut_ptr(), buf.len() as u32, &mut reply_len) };
            String::from_utf8_lossy(&buf[..reply_len as usize]).into_owned()
        } else {
            String::new()
        }
    }))
    .unwrap_or_else(|_| { yog_logging::error!("a mod panicked handling command `{}`", n); String::new() });

    env.new_string(reply).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeItemDefs<'l>(
    env: JNIEnv<'l>, _class: JClass<'l>,
) -> jstring {
    let s = handlers().items.iter().map(|d| {
        let mut parts = vec![d.id.clone()];
        parts.push(format!("max_stack={}", d.max_stack));
        if let Some(n) = &d.name    { parts.push(format!("name={n}")); }
        if let Some(t) = &d.tooltip { parts.push(format!("tooltip={t}")); }
        if d.max_damage > 0         { parts.push(format!("max_damage={}", d.max_damage)); }
        if d.fire_resistant         { parts.push("fire_resistant=1".into()); }
        if d.fuel_ticks > 0         { parts.push(format!("fuel_ticks={}", d.fuel_ticks)); }
        if let Some(f) = &d.food {
            parts.push(format!("food={}:{}:{}", f.nutrition, f.saturation, if f.can_always_eat { 1 } else { 0 }));
        }
        parts.join("\t")
    }).collect::<Vec<_>>().join("\n");
    env.new_string(s).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeBlockDefs<'l>(
    env: JNIEnv<'l>, _class: JClass<'l>,
) -> jstring {
    let s = handlers().blocks.iter().map(|d| {
        let mut parts = vec![d.id.clone()];
        parts.push(format!("hardness={}", d.hardness));
        parts.push(format!("resistance={}", d.resistance));
        if let Some(n) = &d.name { parts.push(format!("name={n}")); }
        if let Some(sh) = d.shape {
            parts.push(format!("shape={}:{}:{}:{}:{}:{}", sh[0], sh[1], sh[2], sh[3], sh[4], sh[5]));
        }
        if d.light_level > 0    { parts.push(format!("light={}", d.light_level)); }
        if let Some(snd) = &d.sound { parts.push(format!("sound={snd}")); }
        if d.requires_tool       { parts.push("requires_tool=1".into()); }
        if d.no_collision        { parts.push("no_collision=1".into()); }
        if d.slipperiness > 0.0  { parts.push(format!("slipperiness={}", d.slipperiness)); }
        parts.join("\t")
    }).collect::<Vec<_>>().join("\n");
    env.new_string(s).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPacket<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    channel: JString<'l>, player: JString<'l>, payload: JByteArray<'l>,
) {
    let ch = env.get_string(&channel).map(String::from).unwrap_or_default();
    let pl = env.get_string(&player).map(String::from).unwrap_or_default();
    let data = env.convert_byte_array(&payload).unwrap_or_default();
    let ev = yog_abi::YogPacketEvent {
        channel: YogStr::from_str(&ch), player: YogStr::from_str(&pl),
        payload: data.as_ptr(), payload_len: data.len() as u32,
    };
    let h = handlers();
    let srv = srv_ptr();
    guard("on_packet", || {
        if let Some((ud, f)) = h.packets.get(&ch) {
            unsafe { f(*ud, srv, &ev) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnClientPacket<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    channel: JString<'l>, payload: JByteArray<'l>,
) {
    let ch = env.get_string(&channel).map(String::from).unwrap_or_default();
    let data = env.convert_byte_array(&payload).unwrap_or_default();
    let ev = yog_abi::YogPacketEvent {
        channel: YogStr::from_str(&ch), player: YogStr::EMPTY,
        payload: data.as_ptr(), payload_len: data.len() as u32,
    };
    let h = handlers();
    let srv = srv_ptr();
    guard("on_client_packet", || {
        if let Some((ud, f)) = h.client_packets.get(&ch) {
            unsafe { f(*ud, srv, &ev) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativePacketChannels<'l>(
    env: JNIEnv<'l>, _class: JClass<'l>,
) -> jstring {
    let s = handlers().packets.keys().cloned().collect::<Vec<_>>().join("\n");
    env.new_string(s).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeClientPacketChannels<'l>(
    env: JNIEnv<'l>, _class: JClass<'l>,
) -> jstring {
    let s = handlers().client_packets.keys().cloned().collect::<Vec<_>>().join("\n");
    env.new_string(s).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntitySpawn<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    entity_type: JString<'l>, uuid: JString<'l>, dimension: JString<'l>,
) {
    let h = handlers();
    if h.entity_spawn.is_empty() { return; }
    let (et, u, d) = (jstr!(env, entity_type), jstr!(env, uuid), jstr!(env, dimension));
    let ev = yog_abi::YogEntitySpawnEvent {
        entity_type: YogStr::from_str(&et), uuid: YogStr::from_str(&u),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    guard("on_entity_spawn", || {
        for (ud, f) in &h.entity_spawn {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntitySpawnPre<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    entity_type: JString<'l>, uuid: JString<'l>, dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.entity_spawn.is_empty() { return 1; }
    let et = match env.get_string(&entity_type) { Ok(s) => String::from(s), Err(_) => return 1 };
    let u  = match env.get_string(&uuid)        { Ok(s) => String::from(s), Err(_) => return 1 };
    let d  = match env.get_string(&dimension)   { Ok(s) => String::from(s), Err(_) => return 1 };
    let ev = yog_abi::YogEntitySpawnEvent {
        entity_type: YogStr::from_str(&et), uuid: YogStr::from_str(&u),
        dimension: YogStr::from_str(&d),
    };
    let srv = srv_ptr();
    let mut allow = true;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for (ud, f) in &h.entity_spawn {
            if !unsafe { f(*ud, srv, &ev, 0) } { allow = false; break; }
        }
    })).ok();
    allow as jni::sys::jboolean
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityDamagePre<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    entity_type: JString<'l>, uuid: JString<'l>, amount: jfloat, source: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.entity_damage.is_empty() { return 1; }
    let et = match env.get_string(&entity_type) { Ok(s) => String::from(s), Err(_) => return 1 };
    let u  = match env.get_string(&uuid)        { Ok(s) => String::from(s), Err(_) => return 1 };
    let s  = match env.get_string(&source)      { Ok(s) => String::from(s), Err(_) => return 1 };
    let ev = yog_abi::YogEntityDamageEvent {
        entity_type: YogStr::from_str(&et), uuid: YogStr::from_str(&u),
        amount, source: YogStr::from_str(&s),
    };
    let srv = srv_ptr();
    let mut allow = true;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for (ud, f) in &h.entity_damage {
            if !unsafe { f(*ud, srv, &ev, 0) } { allow = false; break; }
        }
    })).ok();
    allow as jni::sys::jboolean
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlaceBlockPre<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, block: JString<'l>, x: jint, y: jint, z: jint,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.player_place_block.is_empty() { return 1; }
    let p = match env.get_string(&player) { Ok(s) => String::from(s), Err(_) => return 1 };
    let b = match env.get_string(&block)  { Ok(s) => String::from(s), Err(_) => return 1 };
    let ev = YogPlaceBlockEvent {
        player: YogStr::from_str(&p), block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
    };
    let srv = srv_ptr();
    let mut allow = true;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for (ud, f) in &h.player_place_block {
            if !unsafe { f(*ud, srv, &ev, 0) } { allow = false; break; }
        }
    })).ok();
    allow as jni::sys::jboolean
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlaceBlock<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, block: JString<'l>, x: jint, y: jint, z: jint,
) {
    let h = handlers();
    if h.player_place_block.is_empty() { return; }
    let (p, b) = (jstr!(env, player), jstr!(env, block));
    let ev = YogPlaceBlockEvent {
        player: YogStr::from_str(&p), block: YogStr::from_str(&b),
        pos: YogBlockPos { x, y, z },
    };
    let srv = srv_ptr();
    guard("on_player_place_block", || {
        for (ud, f) in &h.player_place_block {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerDeathPre<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, uuid: JString<'l>, source: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.player_death.is_empty() { return 1; }
    let p  = match env.get_string(&player) { Ok(s) => String::from(s), Err(_) => return 1 };
    let u  = match env.get_string(&uuid)   { Ok(s) => String::from(s), Err(_) => return 1 };
    let s  = match env.get_string(&source) { Ok(s) => String::from(s), Err(_) => return 1 };
    let ev = YogPlayerDeathEvent {
        player: YogStr::from_str(&p), uuid: YogStr::from_str(&u), source: YogStr::from_str(&s),
    };
    let srv = srv_ptr();
    let mut allow = true;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for (ud, f) in &h.player_death {
            if !unsafe { f(*ud, srv, &ev, 0) } { allow = false; break; }
        }
    })).ok();
    allow as jni::sys::jboolean
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerDeath<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, uuid: JString<'l>, source: JString<'l>,
) {
    let h = handlers();
    if h.player_death.is_empty() { return; }
    let (p, u, s) = (jstr!(env, player), jstr!(env, uuid), jstr!(env, source));
    let ev = YogPlayerDeathEvent {
        player: YogStr::from_str(&p), uuid: YogStr::from_str(&u), source: YogStr::from_str(&s),
    };
    let srv = srv_ptr();
    guard("on_player_death", || {
        for (ud, f) in &h.player_death {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerRespawn<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, uuid: JString<'l>, at_anchor: jni::sys::jboolean,
) {
    let h = handlers();
    if h.player_respawn.is_empty() { return; }
    let (p, u) = (jstr!(env, player), jstr!(env, uuid));
    let ev = YogPlayerRespawnEvent {
        player: YogStr::from_str(&p), uuid: YogStr::from_str(&u), at_anchor: at_anchor != 0,
    };
    let srv = srv_ptr();
    guard("on_player_respawn", || {
        for (ud, f) in &h.player_respawn {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnAdvancement<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, uuid: JString<'l>, advancement: JString<'l>,
) {
    let h = handlers();
    if h.advancement.is_empty() { return; }
    let (p, u, a) = (jstr!(env, player), jstr!(env, uuid), jstr!(env, advancement));
    let ev = YogAdvancementEvent {
        player: YogStr::from_str(&p), uuid: YogStr::from_str(&u), advancement: YogStr::from_str(&a),
    };
    let srv = srv_ptr();
    guard("on_advancement", || {
        for (ud, f) in &h.advancement {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityInteractPre<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, player_uuid: JString<'l>,
    entity_type: JString<'l>, entity_uuid: JString<'l>, hand: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.entity_interact.is_empty() { return 1; }
    let p  = match env.get_string(&player)      { Ok(s) => String::from(s), Err(_) => return 1 };
    let pu = match env.get_string(&player_uuid)  { Ok(s) => String::from(s), Err(_) => return 1 };
    let et = match env.get_string(&entity_type)  { Ok(s) => String::from(s), Err(_) => return 1 };
    let eu = match env.get_string(&entity_uuid)  { Ok(s) => String::from(s), Err(_) => return 1 };
    let ha = match env.get_string(&hand)         { Ok(s) => String::from(s), Err(_) => return 1 };
    let ev = YogEntityInteractEvent {
        player: YogStr::from_str(&p), player_uuid: YogStr::from_str(&pu),
        entity_type: YogStr::from_str(&et), entity_uuid: YogStr::from_str(&eu),
        hand: YogStr::from_str(&ha),
    };
    let srv = srv_ptr();
    let mut allow = true;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for (ud, f) in &h.entity_interact {
            if !unsafe { f(*ud, srv, &ev, 0) } { allow = false; break; }
        }
    })).ok();
    allow as jni::sys::jboolean
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityInteract<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, player_uuid: JString<'l>,
    entity_type: JString<'l>, entity_uuid: JString<'l>, hand: JString<'l>,
) {
    let h = handlers();
    if h.entity_interact.is_empty() { return; }
    let (p, pu) = (jstr!(env, player), jstr!(env, player_uuid));
    let (et, eu, ha) = (jstr!(env, entity_type), jstr!(env, entity_uuid), jstr!(env, hand));
    let ev = YogEntityInteractEvent {
        player: YogStr::from_str(&p), player_uuid: YogStr::from_str(&pu),
        entity_type: YogStr::from_str(&et), entity_uuid: YogStr::from_str(&eu),
        hand: YogStr::from_str(&ha),
    };
    let srv = srv_ptr();
    guard("on_entity_interact", || {
        for (ud, f) in &h.entity_interact {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnItemCraft<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, player_uuid: JString<'l>,
    result_item: JString<'l>, result_count: jint,
) {
    let h = handlers();
    if h.item_craft.is_empty() { return; }
    let (p, pu, ri) = (jstr!(env, player), jstr!(env, player_uuid), jstr!(env, result_item));
    let ev = YogCraftEvent {
        player: YogStr::from_str(&p), player_uuid: YogStr::from_str(&pu),
        result_item: YogStr::from_str(&ri), result_count: result_count as u32,
    };
    let srv = srv_ptr();
    guard("on_item_craft", || {
        for (ud, f) in &h.item_craft {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnExplosionPre<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    dimension: JString<'l>, x: jdouble, y: jdouble, z: jdouble,
    power: jfloat, cause_uuid: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.explosion.is_empty() { return 1; }
    let d  = match env.get_string(&dimension)  { Ok(s) => String::from(s), Err(_) => return 1 };
    let cu = match env.get_string(&cause_uuid) { Ok(s) => String::from(s), Err(_) => return 1 };
    let ev = YogExplosionEvent {
        dimension: YogStr::from_str(&d), x, y, z, power, cause_uuid: YogStr::from_str(&cu),
    };
    let srv = srv_ptr();
    let mut allow = true;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for (ud, f) in &h.explosion {
            if !unsafe { f(*ud, srv, &ev, 0) } { allow = false; break; }
        }
    })).ok();
    allow as jni::sys::jboolean
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnExplosion<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    dimension: JString<'l>, x: jdouble, y: jdouble, z: jdouble,
    power: jfloat, cause_uuid: JString<'l>,
) {
    let h = handlers();
    if h.explosion.is_empty() { return; }
    let (d, cu) = (jstr!(env, dimension), jstr!(env, cause_uuid));
    let ev = YogExplosionEvent {
        dimension: YogStr::from_str(&d), x, y, z, power, cause_uuid: YogStr::from_str(&cu),
    };
    let srv = srv_ptr();
    guard("on_explosion", || {
        for (ud, f) in &h.explosion {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

// ── ABI minor 9 JNI entry points ─────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnItemPickupPre<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, player_uuid: JString<'l>,
    item_id: JString<'l>, item_count: jint, entity_uuid: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.item_pickup.is_empty() { return 1; }
    let p   = match env.get_string(&player)      { Ok(s) => String::from(s), Err(_) => return 1 };
    let pu  = match env.get_string(&player_uuid)  { Ok(s) => String::from(s), Err(_) => return 1 };
    let ii  = match env.get_string(&item_id)      { Ok(s) => String::from(s), Err(_) => return 1 };
    let eu  = match env.get_string(&entity_uuid)  { Ok(s) => String::from(s), Err(_) => return 1 };
    let ev = YogItemPickupEvent {
        player: YogStr::from_str(&p), player_uuid: YogStr::from_str(&pu),
        item_id: YogStr::from_str(&ii), item_count: item_count as u32,
        entity_uuid: YogStr::from_str(&eu),
    };
    let srv = srv_ptr();
    let mut allow = true;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for (ud, f) in &h.item_pickup {
            if !unsafe { f(*ud, srv, &ev, 0) } { allow = false; break; }
        }
    })).ok();
    allow as jni::sys::jboolean
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnItemPickup<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, player_uuid: JString<'l>,
    item_id: JString<'l>, item_count: jint, entity_uuid: JString<'l>,
) {
    let h = handlers();
    if h.item_pickup.is_empty() { return; }
    let (p, pu) = (jstr!(env, player), jstr!(env, player_uuid));
    let (ii, eu) = (jstr!(env, item_id), jstr!(env, entity_uuid));
    let ev = YogItemPickupEvent {
        player: YogStr::from_str(&p), player_uuid: YogStr::from_str(&pu),
        item_id: YogStr::from_str(&ii), item_count: item_count as u32,
        entity_uuid: YogStr::from_str(&eu),
    };
    let srv = srv_ptr();
    guard("on_item_pickup", || {
        for (ud, f) in &h.item_pickup {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerMove<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, player_uuid: JString<'l>,
    x: jdouble, y: jdouble, z: jdouble, yaw: jfloat, pitch: jfloat,
) {
    let h = handlers();
    if h.player_move.is_empty() { return; }
    let (p, pu) = (jstr!(env, player), jstr!(env, player_uuid));
    let ev = YogPlayerMoveEvent {
        player: YogStr::from_str(&p), player_uuid: YogStr::from_str(&pu),
        x, y, z, yaw, pitch,
    };
    let srv = srv_ptr();
    guard("on_player_move", || {
        for (ud, f) in &h.player_move {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnContainerOpenPre<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, player_uuid: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.container_open.is_empty() { return 1; }
    let p  = match env.get_string(&player)      { Ok(s) => String::from(s), Err(_) => return 1 };
    let pu = match env.get_string(&player_uuid)  { Ok(s) => String::from(s), Err(_) => return 1 };
    let ev = YogContainerOpenEvent {
        player: YogStr::from_str(&p), player_uuid: YogStr::from_str(&pu),
        container_type: YogStr::EMPTY,
    };
    let srv = srv_ptr();
    let mut allow = true;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for (ud, f) in &h.container_open {
            if !unsafe { f(*ud, srv, &ev, 0) } { allow = false; break; }
        }
    })).ok();
    allow as jni::sys::jboolean
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnContainerOpen<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, player_uuid: JString<'l>, container_type: JString<'l>,
) {
    let h = handlers();
    if h.container_open.is_empty() { return; }
    let (p, pu, ct) = (jstr!(env, player), jstr!(env, player_uuid), jstr!(env, container_type));
    let ev = YogContainerOpenEvent {
        player: YogStr::from_str(&p), player_uuid: YogStr::from_str(&pu),
        container_type: YogStr::from_str(&ct),
    };
    let srv = srv_ptr();
    guard("on_container_open", || {
        for (ud, f) in &h.container_open {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnContainerClose<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    player: JString<'l>, player_uuid: JString<'l>,
) {
    let h = handlers();
    if h.container_close.is_empty() { return; }
    let (p, pu) = (jstr!(env, player), jstr!(env, player_uuid));
    let ev = YogContainerCloseEvent {
        player: YogStr::from_str(&p), player_uuid: YogStr::from_str(&pu),
    };
    let srv = srv_ptr();
    guard("on_container_close", || {
        for (ud, f) in &h.container_close {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnProjectileHitPre<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    projectile_type: JString<'l>, projectile_uuid: JString<'l>, shooter_uuid: JString<'l>,
    hit_type: JString<'l>, hit_entity_uuid: JString<'l>,
    x: jdouble, y: jdouble, z: jdouble, dimension: JString<'l>,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.projectile_hit.is_empty() { return 1; }
    let pt  = match env.get_string(&projectile_type)  { Ok(s) => String::from(s), Err(_) => return 1 };
    let pu  = match env.get_string(&projectile_uuid)  { Ok(s) => String::from(s), Err(_) => return 1 };
    let su  = match env.get_string(&shooter_uuid)     { Ok(s) => String::from(s), Err(_) => return 1 };
    let ht  = match env.get_string(&hit_type)         { Ok(s) => String::from(s), Err(_) => return 1 };
    let heu = match env.get_string(&hit_entity_uuid)  { Ok(s) => String::from(s), Err(_) => return 1 };
    let dim = match env.get_string(&dimension)        { Ok(s) => String::from(s), Err(_) => return 1 };
    let ev = YogProjectileHitEvent {
        projectile_type: YogStr::from_str(&pt), projectile_uuid: YogStr::from_str(&pu),
        shooter_uuid: YogStr::from_str(&su), hit_type: YogStr::from_str(&ht),
        hit_entity_uuid: YogStr::from_str(&heu), x, y, z, dimension: YogStr::from_str(&dim),
    };
    let srv = srv_ptr();
    let mut allow = true;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for (ud, f) in &h.projectile_hit {
            if !unsafe { f(*ud, srv, &ev, 0) } { allow = false; break; }
        }
    })).ok();
    allow as jni::sys::jboolean
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnProjectileHit<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    projectile_type: JString<'l>, projectile_uuid: JString<'l>, shooter_uuid: JString<'l>,
    hit_type: JString<'l>, hit_entity_uuid: JString<'l>,
    x: jdouble, y: jdouble, z: jdouble, dimension: JString<'l>,
) {
    let h = handlers();
    if h.projectile_hit.is_empty() { return; }
    let (pt, pu) = (jstr!(env, projectile_type), jstr!(env, projectile_uuid));
    let (su, ht) = (jstr!(env, shooter_uuid), jstr!(env, hit_type));
    let (heu, dim) = (jstr!(env, hit_entity_uuid), jstr!(env, dimension));
    let ev = YogProjectileHitEvent {
        projectile_type: YogStr::from_str(&pt), projectile_uuid: YogStr::from_str(&pu),
        shooter_uuid: YogStr::from_str(&su), hit_type: YogStr::from_str(&ht),
        hit_entity_uuid: YogStr::from_str(&heu), x, y, z, dimension: YogStr::from_str(&dim),
    };
    let srv = srv_ptr();
    guard("on_projectile_hit", || {
        for (ud, f) in &h.projectile_hit {
            unsafe { f(*ud, srv, &ev, 1) };
        }
    });
}

// ── ABI minor 10 — client-side JNI entry points ───────────────────────────────

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnClientTick<'l>(
    _env: JNIEnv<'l>, _class: JClass<'l>,
) {
    let h = handlers();
    if h.client_tick.is_empty() { return; }
    guard("on_client_tick", || {
        for (ud, f) in &h.client_tick {
            unsafe { f(*ud) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeGlInit<'l>(
    _env: JNIEnv<'l>, _class: JClass<'l>,
) {
    if GL.get().is_some() { return; }
    let mut raw_get_binary: usize = 0;
    let mut raw_prog_binary: usize = 0;
    let gl = unsafe {
        glow::Context::from_loader_function(|sym| {
            let Some(mut env) = get_env() else { return std::ptr::null() };
            let jsym = match env.new_string(sym) {
                Ok(s) => s,
                Err(_) => return std::ptr::null(),
            };
            let jsym_obj: JObject = jsym.into();
            let val = env.call_static_method(
                "dev/yog/NativeBridge",
                "glProcAddress",
                "(Ljava/lang/String;)J",
                &[JValue::Object(&jsym_obj)],
            );
            let ptr = match val.and_then(|v| v.j()) {
                Ok(p) if p != 0 => p as usize as *const _,
                _ => std::ptr::null(),
            };
            // Capture extension pointers while the loader runs.
            match sym {
                "glGetProgramBinary" => raw_get_binary = ptr as usize,
                "glProgramBinary"    => raw_prog_binary = ptr as usize,
                _ => {}
            }
            ptr
        })
    };
    let _ = GL.set(GlCtx(gl));
    let _ = GL_GET_PROGRAM_BINARY.set(if raw_get_binary != 0 { Some(raw_get_binary) } else { None });
    let _ = GL_PROGRAM_BINARY.set(if raw_prog_binary != 0 { Some(raw_prog_binary) } else { None });
    // `glGetProgramiv` is a core function; always available.  We look it up once here
    // to avoid depending on glow internals for the PROGRAM_BINARY_LENGTH query.
    if let Some(mut env) = get_env() {
        if let Ok(jsym) = env.new_string("glGetProgramiv") {
            let jsym_obj: JObject = jsym.into();
            if let Ok(jv) = env.call_static_method(
                "dev/yog/NativeBridge", "glProcAddress", "(Ljava/lang/String;)J",
                &[JValue::Object(&jsym_obj)],
            ) {
                if let Ok(ptr) = jv.j() {
                    let _ = GL_GET_PROGRAM_IV.set(if ptr != 0 { Some(ptr as usize) } else { None });
                }
            }
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnHudRender<'l>(
    _env: JNIEnv<'l>, _class: JClass<'l>,
    delta_tick: jfloat,
    screen_w: jint,
    screen_h: jint,
    scale_factor: jfloat,
    player_x: jfloat, player_y: jfloat, player_z: jfloat,
) {
    let h = handlers();
    if h.hud_render.is_empty() { return; }
    let mut gfx = GFX_FN_TABLE;
    gfx.screen_w = screen_w;
    gfx.screen_h = screen_h;
    gfx.delta_tick = delta_tick;
    gfx.scale_factor = scale_factor;
    gfx.player_pos = [player_x, player_y, player_z];
    guard("on_hud_render", || {
        for (ud, f) in &h.hud_render {
            unsafe { f(*ud, &gfx) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnWorldRender<'l>(
    env: JNIEnv<'l>, _class: JClass<'l>,
    delta_tick: jfloat,
    screen_w: jint,
    screen_h: jint,
    scale_factor: jfloat,
    view_proj_arr: JFloatArray<'l>,
    cam_x: jfloat, cam_y: jfloat, cam_z: jfloat,
    player_x: jfloat, player_y: jfloat, player_z: jfloat,
) {
    let h = handlers();
    if h.world_render.is_empty() { return; }
    let mut view_proj = [0f32; 16];
    if env.get_float_array_region(&view_proj_arr, 0, &mut view_proj).is_err() { return; }
    let mut gfx = GFX_FN_TABLE;
    gfx.screen_w = screen_w;
    gfx.screen_h = screen_h;
    gfx.delta_tick = delta_tick;
    gfx.scale_factor = scale_factor;
    gfx.view_proj = view_proj;
    gfx.camera_pos = [cam_x, cam_y, cam_z];
    gfx.player_pos = [player_x, player_y, player_z];
    guard("on_world_render", || {
        for (ud, f) in &h.world_render {
            unsafe { f(*ud, &gfx) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnKeyPress<'l>(
    _env: JNIEnv<'l>, _class: JClass<'l>,
    key_code: jint, scan_code: jint, action: jint, modifiers: jint,
) -> jni::sys::jboolean {
    let h = handlers();
    if h.key_press.is_empty() { return 1; }
    let ev = YogKeyPressEvent { key_code, scan_code, action, modifiers };
    let mut allow = true;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for (ud, f) in &h.key_press {
            if !unsafe { f(*ud, &ev) } { allow = false; break; }
        }
    })).ok();
    allow as jni::sys::jboolean
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnScreenOpen<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    screen_class: JString<'l>,
) {
    let h = handlers();
    if h.screen_open.is_empty() { return; }
    let sc = match env.get_string(&screen_class) { Ok(s) => String::from(s), Err(_) => return };
    guard("on_screen_open", || {
        for (ud, f) in &h.screen_open {
            unsafe { f(*ud, YogStr::from_str(&sc)) };
        }
    });
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnScreenClose<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    screen_class: JString<'l>,
) {
    let h = handlers();
    if h.screen_close.is_empty() { return; }
    let sc = match env.get_string(&screen_class) { Ok(s) => String::from(s), Err(_) => return };
    guard("on_screen_close", || {
        for (ud, f) in &h.screen_close {
            unsafe { f(*ud, YogStr::from_str(&sc)) };
        }
    });
}


// ── UI system JNI ─────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIShow<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>, ui_id: JString<'l>, _w: jint, _h: jint,
) {
    let id = jstr!(env, ui_id);
    yog_logging::info!("UI show: {}", id);
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIHide<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>, ui_id: JString<'l>,
) {
    let id = jstr!(env, ui_id);
    yog_logging::info!("UI hide: {}", id);
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIClick<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    ui_id: JString<'l>, mx: jfloat, my: jfloat, button: jint,
) {
    let id = jstr!(env, ui_id);
    let h = handlers();
    if let Some((ud, handler)) = h.ui_handlers.get(&id).copied() {
        if let Some(ui_root) = h.uis.get(&id) {
            if let Some(hit) = yog_ui::layout::hit_test(ui_root, mx, my) {
                if let Some(event) = &hit.on_click {
                    yog_logging::info!("UI click '{}' → event '{}'", id, event);
                    let ev = YogStr::from_str(event);
                    let ui = YogStr::from_str(&id);
                    unsafe { handler(ud, ui, ev); }
                }
            }
        }
    }
    let _ = button;
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIKey<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>,
    ui_id: JString<'l>, key: jint, _scan: jint, _mods: jint, action: jint,
) {
    let id = jstr!(env, ui_id);
    yog_logging::info!("UI key: {} key={} action={}", id, key, action);
}

#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeUIRender<'l>(
    mut env: JNIEnv<'l>, _class: JClass<'l>, ui_id: JString<'l>,
) {
    let id = jstr!(env, ui_id);
    // The mod's on_hud_render handler will call ui.render(ctx)
    // So this just triggers a repaint — the mod handles actual rendering.
    let _ = id;
}