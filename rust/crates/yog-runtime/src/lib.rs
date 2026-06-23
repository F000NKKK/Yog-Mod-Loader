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
use std::os::raw::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use jni::objects::{JByteArray, JClass, JString, JValue};
use jni::sys::{jdouble, jfloat, jint, jstring};
use jni::{JNIEnv, JavaVM};
use libloading::{Library, Symbol};

use yog_abi::{
    ABI_VERSION, YogAdvancementEvent, YogAdvancementFn, YogApi, YogAttackEntityFn,
    YogBlockBreakFn, YogBlockDef, YogBlockPos, YogChatFn, YogClientFn, YogCommandFn,
    YogContainerCloseEvent, YogContainerCloseFn, YogContainerOpenEvent, YogContainerOpenFn,
    YogCraftEvent, YogCraftFn, YogEntityDamageFn, YogEntityDeathFn, YogEntityInteractEvent,
    YogEntityInteractFn, YogEntitySpawnFn, YogExplosionEvent, YogExplosionFn, YogHudRenderFn,
    YogItemDef, YogItemPickupEvent, YogItemPickupFn, YogKeyPressFn, YogKeyPressEvent,
    YogOwnedStr, YogPacketFn, YogPlaceBlockEvent, YogPlaceBlockFn, YogPlayerDeathEvent,
    YogPlayerDeathFn, YogPlayerFn, YogPlayerMoveEvent, YogPlayerMoveFn, YogPlayerRespawnEvent,
    YogPlayerRespawnFn, YogProjectileHitEvent, YogProjectileHitFn, YogScheduledFn, YogScreenFn,
    YogServer, YogServerFn, YogStr, YogUseBlockFn, YogUseItemFn, YogVec3,
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
            client_tick: Vec::new(), hud_render: Vec::new(), key_press: Vec::new(),
            screen_open: Vec::new(), screen_close: Vec::new(),
            server_tick: Vec::new(), server_started: Vec::new(), server_stopping: Vec::new(),
            commands: HashMap::new(), typed_schemas: HashMap::new(),
            recipes: Vec::new(), packets: HashMap::new(),
            client_packets: HashMap::new(), items: Vec::new(),
            blocks: Vec::new(), scheduler: Mutex::new(SchedulerState::new()),
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
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnHudRender<'l>(
    _env: JNIEnv<'l>, _class: JClass<'l>,
    delta_tick: jfloat,
) {
    let h = handlers();
    if h.hud_render.is_empty() { return; }
    guard("on_hud_render", || {
        for (ud, f) in &h.hud_render {
            unsafe { f(*ud, delta_tick) };
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
