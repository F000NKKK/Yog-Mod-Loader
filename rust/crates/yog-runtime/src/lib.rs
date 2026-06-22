//! Yog runtime — the native library loaded by the Fabric host.
//!
//! It exposes JNI entry points that the Java side calls, translates the incoming
//! data into [`yog_api`] events, and dispatches them to registered Rust mods.
//! It also implements [`yog_api::Server`], the Rust → Minecraft path, by calling
//! back into the Java host through a cached [`JavaVM`].
//!
//! Symbol naming follows the JNI convention `Java_<package>_<class>_<method>`,
//! here `dev.yog.NativeBridge`.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, RwLock};

use jni::objects::{JByteArray, JClass, JString, JValue};
use jni::sys::{jfloat, jint, jstring};
use jni::{JNIEnv, JavaVM};
use libloading::{Library, Symbol};

use yog_api::{
    AttackEntityEvent, BlockBreakEvent, BlockPos, ChatEvent, CommandContext, EntityDamageEvent,
    EntityDeathEvent, PacketEvent, PlayerJoinEvent, PlayerLeaveEvent, Registry, Server,
    UseBlockEvent, UseItemEvent, ABI_VERSION,
};

/// Loaded mod libraries, kept alive for the process so their code stays mapped.
static LOADED_MODS: Mutex<Vec<Library>> = Mutex::new(Vec::new());

type AbiVersionFn = unsafe extern "C" fn() -> u32;
type RegisterFn = unsafe extern "C" fn(*mut Registry);

/// Platform tag matching `std::env::consts`, e.g. `linux-x86_64`. Mirrors the
/// layout the host uses for embedded natives and `.yog` mods.
fn platform_tag() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// Load every mod in `dir`: plain native libs (`.so`/`.dll`/`.dylib`) and `.yog`
/// archives (from which the current platform's native is extracted first).
fn load_mods(dir: &Path, registry: &mut Registry) {
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
        if load_mod_lib(&lib_path, registry) {
            count += 1;
        }
    }
    yog_logging::info!("loaded {} mod(s) from {}", count, dir.display());
}

/// dlopen one native lib, verify its ABI, and run its `yog_mod_register`.
fn load_mod_lib(path: &Path, registry: &mut Registry) -> bool {
    unsafe {
        let lib = match Library::new(path) {
            Ok(l) => l,
            Err(e) => {
                yog_logging::error!("failed to load {}: {}", path.display(), e);
                return false;
            }
        };
        let abi: Symbol<AbiVersionFn> = match lib.get(b"yog_abi_version") {
            Ok(s) => s,
            Err(_) => {
                yog_logging::error!("{} is not a Yog mod (no yog_abi_version)", path.display());
                return false;
            }
        };
        let mod_abi = abi();
        if mod_abi != ABI_VERSION {
            yog_logging::error!(
                "{}: ABI {} incompatible with runtime ABI {}",
                path.display(),
                mod_abi,
                ABI_VERSION
            );
            return false;
        }
        let register: Symbol<RegisterFn> = match lib.get(b"yog_mod_register") {
            Ok(s) => s,
            Err(_) => {
                yog_logging::error!("{} missing yog_mod_register", path.display());
                return false;
            }
        };
        register(registry as *mut Registry);
        drop(register);
        drop(abi);
        LOADED_MODS.lock().expect("mods lock poisoned").push(lib);
    }
    true
}

/// Extract the current platform's native from a `.yog` archive to a temp file.
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

    let ext = Path::new(&entry_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let out = std::env::temp_dir().join(format!("yog-{}-{}.{}", stem, std::process::id(), ext));

    let mut entry = archive.by_name(&entry_name).ok()?;
    let mut out_file = std::fs::File::create(&out).ok()?;
    std::io::copy(&mut entry, &mut out_file).ok()?;
    Some(out)
}

/// Global registry of mod event handlers, initialised once on startup.
static REGISTRY: OnceLock<RwLock<Registry>> = OnceLock::new();

/// Cached VM handle so we can call back into Java from any thread.
static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();

fn registry() -> &'static RwLock<Registry> {
    REGISTRY.get_or_init(|| RwLock::new(Registry::default()))
}

/// Run `f`, catching any panic that would otherwise unwind across the JNI
/// boundary into the JVM (undefined behaviour). One misbehaving mod must not
/// crash the server, so the panic is logged and swallowed.
fn guard(label: &str, f: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err() {
        yog_logging::error!("a mod panicked handling `{}` (ignored)", label);
    }
}

/// Read a `JString` argument into a Rust `String`, returning early on error.
macro_rules! jstr {
    ($env:expr, $s:expr) => {
        match $env.get_string(&$s) {
            Ok(s) => String::from(s),
            Err(_) => return,
        }
    };
}

/// Concrete [`Server`] handed to handlers. Calls into `dev.yog.NativeBridge`
/// static methods via the cached [`JavaVM`].
struct JniServer;

impl Server for JniServer {
    fn broadcast(&self, message: &str) {
        let Some(vm) = JAVA_VM.get() else {
            return;
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            return;
        };
        let Ok(jmsg) = env.new_string(message) else {
            return;
        };
        let _ = env.call_static_method(
            "dev/yog/NativeBridge",
            "broadcast",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&jmsg)],
        );
    }

    fn get_block(&self, dimension: &str, pos: BlockPos) -> Option<String> {
        let vm = JAVA_VM.get()?;
        let mut env = vm.attach_current_thread().ok()?;
        let jdim = env.new_string(dimension).ok()?;
        let ret = env
            .call_static_method(
                "dev/yog/NativeBridge",
                "getBlock",
                "(Ljava/lang/String;III)Ljava/lang/String;",
                &[
                    JValue::Object(&jdim),
                    JValue::Int(pos.x),
                    JValue::Int(pos.y),
                    JValue::Int(pos.z),
                ],
            )
            .ok()?;
        let obj = ret.l().ok()?;
        if obj.as_raw().is_null() {
            return None;
        }
        let jstr = JString::from(obj);
        let block_id: String = env.get_string(&jstr).ok()?.into();
        Some(block_id)
    }

    fn set_block(&self, dimension: &str, pos: BlockPos, block_id: &str) -> bool {
        let Some(vm) = JAVA_VM.get() else {
            return false;
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            return false;
        };
        let (Ok(jdim), Ok(jid)) = (env.new_string(dimension), env.new_string(block_id)) else {
            return false;
        };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "setBlock",
            "(Ljava/lang/String;IIILjava/lang/String;)Z",
            &[
                JValue::Object(&jdim),
                JValue::Int(pos.x),
                JValue::Int(pos.y),
                JValue::Int(pos.z),
                JValue::Object(&jid),
            ],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn give_item(&self, player: &str, item_id: &str, count: u32) -> bool {
        let Some(vm) = JAVA_VM.get() else {
            return false;
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            return false;
        };
        let (Ok(jp), Ok(ji)) = (env.new_string(player), env.new_string(item_id)) else {
            return false;
        };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "giveItem",
            "(Ljava/lang/String;Ljava/lang/String;I)Z",
            &[JValue::Object(&jp), JValue::Object(&ji), JValue::Int(count as i32)],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn teleport(&self, player: &str, x: f64, y: f64, z: f64) -> bool {
        let Some(vm) = JAVA_VM.get() else {
            return false;
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            return false;
        };
        let Ok(jp) = env.new_string(player) else {
            return false;
        };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "teleport",
            "(Ljava/lang/String;DDD)Z",
            &[
                JValue::Object(&jp),
                JValue::Double(x),
                JValue::Double(y),
                JValue::Double(z),
            ],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn send_to_player(&self, player: &str, channel: &str, payload: &[u8]) -> bool {
        let Some(vm) = JAVA_VM.get() else {
            return false;
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            return false;
        };
        let (Ok(jp), Ok(jc), Ok(data)) = (
            env.new_string(player),
            env.new_string(channel),
            env.byte_array_from_slice(payload),
        ) else {
            return false;
        };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "sendToPlayer",
            "(Ljava/lang/String;Ljava/lang/String;[B)Z",
            &[JValue::Object(&jp), JValue::Object(&jc), JValue::Object(&data)],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn send_to_server(&self, channel: &str, payload: &[u8]) -> bool {
        let Some(vm) = JAVA_VM.get() else {
            return false;
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            return false;
        };
        let (Ok(jc), Ok(data)) = (env.new_string(channel), env.byte_array_from_slice(payload))
        else {
            return false;
        };
        let result = env.call_static_method(
            "dev/yog/YogClient",
            "sendToServer",
            "(Ljava/lang/String;[B)Z",
            &[JValue::Object(&jc), JValue::Object(&data)],
        );
        // YogClient is client-only: on a dedicated server the class is absent and
        // the call leaves a pending exception — clear it and report failure.
        let _ = env.exception_clear();
        result.and_then(|v| v.z()).unwrap_or(false)
    }

    fn entity_teleport(&self, uuid: &str, x: f64, y: f64, z: f64) -> bool {
        let Some(vm) = JAVA_VM.get() else {
            return false;
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            return false;
        };
        let Ok(ju) = env.new_string(uuid) else {
            return false;
        };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "entityTeleport",
            "(Ljava/lang/String;DDD)Z",
            &[
                JValue::Object(&ju),
                JValue::Double(x),
                JValue::Double(y),
                JValue::Double(z),
            ],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn entity_position(&self, uuid: &str) -> Option<(f64, f64, f64)> {
        let vm = JAVA_VM.get()?;
        let mut env = vm.attach_current_thread().ok()?;
        let ju = env.new_string(uuid).ok()?;
        let ret = env
            .call_static_method(
                "dev/yog/NativeBridge",
                "entityPosition",
                "(Ljava/lang/String;)Ljava/lang/String;",
                &[JValue::Object(&ju)],
            )
            .ok()?;
        let obj = ret.l().ok()?;
        if obj.as_raw().is_null() {
            return None;
        }
        let jstr = JString::from(obj);
        let s: String = env.get_string(&jstr).ok()?.into();
        let mut it = s.split('\t');
        let x: f64 = it.next()?.parse().ok()?;
        let y: f64 = it.next()?.parse().ok()?;
        let z: f64 = it.next()?.parse().ok()?;
        Some((x, y, z))
    }

    fn entity_health(&self, uuid: &str) -> Option<f32> {
        let vm = JAVA_VM.get()?;
        let mut env = vm.attach_current_thread().ok()?;
        let ju = env.new_string(uuid).ok()?;
        let v = env
            .call_static_method(
                "dev/yog/NativeBridge",
                "entityHealth",
                "(Ljava/lang/String;)D",
                &[JValue::Object(&ju)],
            )
            .ok()?
            .d()
            .ok()?;
        if v.is_nan() {
            None
        } else {
            Some(v as f32)
        }
    }

    fn entity_set_health(&self, uuid: &str, health: f32) -> bool {
        let Some(vm) = JAVA_VM.get() else {
            return false;
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            return false;
        };
        let Ok(ju) = env.new_string(uuid) else {
            return false;
        };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "entitySetHealth",
            "(Ljava/lang/String;D)Z",
            &[JValue::Object(&ju), JValue::Double(health as f64)],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn entity_kill(&self, uuid: &str) -> bool {
        let Some(vm) = JAVA_VM.get() else {
            return false;
        };
        let Ok(mut env) = vm.attach_current_thread() else {
            return false;
        };
        let Ok(ju) = env.new_string(uuid) else {
            return false;
        };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "entityKill",
            "(Ljava/lang/String;)Z",
            &[JValue::Object(&ju)],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn world_time(&self, dimension: &str) -> Option<i64> {
        let vm = JAVA_VM.get()?;
        let mut env = vm.attach_current_thread().ok()?;
        let jd = env.new_string(dimension).ok()?;
        let v = env
            .call_static_method(
                "dev/yog/NativeBridge",
                "worldTime",
                "(Ljava/lang/String;)J",
                &[JValue::Object(&jd)],
            )
            .ok()?
            .j()
            .ok()?;
        if v == i64::MIN { None } else { Some(v) }
    }

    fn world_set_time(&self, dimension: &str, time: i64) -> bool {
        let Some(vm) = JAVA_VM.get() else { return false; };
        let Ok(mut env) = vm.attach_current_thread() else { return false; };
        let Ok(jd) = env.new_string(dimension) else { return false; };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "worldSetTime",
            "(Ljava/lang/String;J)Z",
            &[JValue::Object(&jd), JValue::Long(time)],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn world_is_raining(&self, dimension: &str) -> bool {
        let Some(vm) = JAVA_VM.get() else { return false; };
        let Ok(mut env) = vm.attach_current_thread() else { return false; };
        let Ok(jd) = env.new_string(dimension) else { return false; };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "worldIsRaining",
            "(Ljava/lang/String;)Z",
            &[JValue::Object(&jd)],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn world_set_weather(&self, dimension: &str, raining: bool, duration_ticks: i32) -> bool {
        let Some(vm) = JAVA_VM.get() else { return false; };
        let Ok(mut env) = vm.attach_current_thread() else { return false; };
        let Ok(jd) = env.new_string(dimension) else { return false; };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "worldSetWeather",
            "(Ljava/lang/String;ZI)Z",
            &[JValue::Object(&jd), JValue::Bool(raining as u8), JValue::Int(duration_ticks)],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn entity_add_effect(
        &self,
        uuid: &str,
        effect_id: &str,
        duration_ticks: i32,
        amplifier: u8,
        show_particles: bool,
    ) -> bool {
        let Some(vm) = JAVA_VM.get() else { return false; };
        let Ok(mut env) = vm.attach_current_thread() else { return false; };
        let (Ok(ju), Ok(je)) = (env.new_string(uuid), env.new_string(effect_id)) else { return false; };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "entityAddEffect",
            "(Ljava/lang/String;Ljava/lang/String;IIZ)Z",
            &[
                JValue::Object(&ju),
                JValue::Object(&je),
                JValue::Int(duration_ticks),
                JValue::Int(amplifier as i32),
                JValue::Bool(show_particles as u8),
            ],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn entity_remove_effect(&self, uuid: &str, effect_id: &str) -> bool {
        let Some(vm) = JAVA_VM.get() else { return false; };
        let Ok(mut env) = vm.attach_current_thread() else { return false; };
        let (Ok(ju), Ok(je)) = (env.new_string(uuid), env.new_string(effect_id)) else { return false; };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "entityRemoveEffect",
            "(Ljava/lang/String;Ljava/lang/String;)Z",
            &[JValue::Object(&ju), JValue::Object(&je)],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn entity_clear_effects(&self, uuid: &str) -> bool {
        let Some(vm) = JAVA_VM.get() else { return false; };
        let Ok(mut env) = vm.attach_current_thread() else { return false; };
        let Ok(ju) = env.new_string(uuid) else { return false; };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "entityClearEffects",
            "(Ljava/lang/String;)Z",
            &[JValue::Object(&ju)],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn drop_loot(&self, table_id: &str, dimension: &str, x: f64, y: f64, z: f64) -> bool {
        let Some(vm) = JAVA_VM.get() else { return false; };
        let Ok(mut env) = vm.attach_current_thread() else { return false; };
        let (Ok(jt), Ok(jd)) = (env.new_string(table_id), env.new_string(dimension)) else { return false; };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "dropLoot",
            "(Ljava/lang/String;Ljava/lang/String;DDD)Z",
            &[
                JValue::Object(&jt),
                JValue::Object(&jd),
                JValue::Double(x),
                JValue::Double(y),
                JValue::Double(z),
            ],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn has_item_tag(&self, item_id: &str, tag_id: &str) -> bool {
        let Some(vm) = JAVA_VM.get() else { return false; };
        let Ok(mut env) = vm.attach_current_thread() else { return false; };
        let (Ok(ji), Ok(jt)) = (env.new_string(item_id), env.new_string(tag_id)) else { return false; };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "hasItemTag",
            "(Ljava/lang/String;Ljava/lang/String;)Z",
            &[JValue::Object(&ji), JValue::Object(&jt)],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn has_block_tag(&self, block_id: &str, tag_id: &str) -> bool {
        let Some(vm) = JAVA_VM.get() else { return false; };
        let Ok(mut env) = vm.attach_current_thread() else { return false; };
        let (Ok(jb), Ok(jt)) = (env.new_string(block_id), env.new_string(tag_id)) else { return false; };
        env.call_static_method(
            "dev/yog/NativeBridge",
            "hasBlockTag",
            "(Ljava/lang/String;Ljava/lang/String;)Z",
            &[JValue::Object(&jb), JValue::Object(&jt)],
        )
        .and_then(|v| v.z())
        .unwrap_or(false)
    }

    fn spawn_entity(
        &self,
        entity_type: &str,
        dimension: &str,
        x: f64,
        y: f64,
        z: f64,
    ) -> Option<String> {
        let vm = JAVA_VM.get()?;
        let mut env = vm.attach_current_thread().ok()?;
        let jt = env.new_string(entity_type).ok()?;
        let jd = env.new_string(dimension).ok()?;
        let ret = env
            .call_static_method(
                "dev/yog/NativeBridge",
                "spawnEntity",
                "(Ljava/lang/String;Ljava/lang/String;DDD)Ljava/lang/String;",
                &[
                    JValue::Object(&jt),
                    JValue::Object(&jd),
                    JValue::Double(x),
                    JValue::Double(y),
                    JValue::Double(z),
                ],
            )
            .ok()?;
        let obj = ret.l().ok()?;
        if obj.as_raw().is_null() {
            return None;
        }
        let jstr = JString::from(obj);
        let uuid: String = env.get_string(&jstr).ok()?.into();
        Some(uuid)
    }
}

/// Called once by the Java host after the native library is loaded. `mods_dir`
/// is the directory scanned for `.yog` / native mods.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeInit<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    mods_dir: JString<'l>,
) {
    if let Ok(vm) = env.get_java_vm() {
        let _ = JAVA_VM.set(vm);
    }

    let dir = env.get_string(&mods_dir).map(String::from).unwrap_or_default();

    guard("mod loading", || {
        let mut reg = registry().write().expect("registry poisoned");
        load_mods(Path::new(&dir), &mut reg);
    });

    yog_logging::info!("runtime initialised — the gate is open.");
}

/// Called by the host (Fabric `PlayerBlockBreakEvents`) when a player breaks a
/// block, server side.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnBlockBreak<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    block: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
) {
    let event = BlockBreakEvent {
        player_name: jstr!(env, player),
        block_id: jstr!(env, block),
        pos: BlockPos { x, y, z },
    };

    guard("on_block_break", || {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_block_break(&event, &JniServer);
    });
}

/// Called by the host when a player sends a chat message.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnChat<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    message: JString<'l>,
) {
    let event = ChatEvent {
        player_name: jstr!(env, player),
        message: jstr!(env, message),
    };

    guard("on_chat", || {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_chat(&event, &JniServer);
    });
}

/// Called by the host when a player joins the server.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerJoin<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    uuid: JString<'l>,
) {
    let event = PlayerJoinEvent {
        player_name: jstr!(env, player),
        uuid: jstr!(env, uuid),
    };

    guard("on_player_join", || {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_player_join(&event, &JniServer);
    });
}

/// Called by the host when a player leaves the server.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPlayerLeave<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    uuid: JString<'l>,
) {
    let event = PlayerLeaveEvent {
        player_name: jstr!(env, player),
        uuid: jstr!(env, uuid),
    };

    guard("on_player_leave", || {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_player_leave(&event, &JniServer);
    });
}

/// Called by the host when a player right-clicks with an item (server side).
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnUseItem<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    item: JString<'l>,
) {
    let event = UseItemEvent {
        player_name: jstr!(env, player),
        item_id: jstr!(env, item),
    };
    guard("on_use_item", || {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_use_item(&event, &JniServer);
    });
}

/// Called by the host when a player right-clicks a block (server side).
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnUseBlock<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    block: JString<'l>,
    x: jint,
    y: jint,
    z: jint,
) {
    let event = UseBlockEvent {
        player_name: jstr!(env, player),
        block_id: jstr!(env, block),
        pos: BlockPos { x, y, z },
    };
    guard("on_use_block", || {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_use_block(&event, &JniServer);
    });
}

/// Called by the host when a player attacks (left-clicks) an entity.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnAttackEntity<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    player: JString<'l>,
    target_type: JString<'l>,
    target_uuid: JString<'l>,
) {
    let event = AttackEntityEvent {
        player_name: jstr!(env, player),
        target_type: jstr!(env, target_type),
        target_uuid: jstr!(env, target_uuid),
    };
    guard("on_attack_entity", || {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_attack_entity(&event, &JniServer);
    });
}

/// Called by the host after a living entity takes damage.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityDamage<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    entity_type: JString<'l>,
    uuid: JString<'l>,
    amount: jfloat,
    source: JString<'l>,
) {
    let event = EntityDamageEvent {
        entity_type: jstr!(env, entity_type),
        uuid: jstr!(env, uuid),
        amount,
        source: jstr!(env, source),
    };
    guard("on_entity_damage", || {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_entity_damage(&event, &JniServer);
    });
}

/// Called by the host after a living entity dies.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnEntityDeath<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    entity_type: JString<'l>,
    uuid: JString<'l>,
    source: JString<'l>,
) {
    let event = EntityDeathEvent {
        entity_type: jstr!(env, entity_type),
        uuid: jstr!(env, uuid),
        source: jstr!(env, source),
    };
    guard("on_entity_death", || {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_entity_death(&event, &JniServer);
    });
}

/// Called by the host at the end of every server tick.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnTick<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    guard("on_tick", || {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_server_tick(&JniServer);
    });
}

/// Called by the host once the server has finished starting.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnServerStarted<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    guard("on_server_started", || {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_server_started(&JniServer);
    });
}

/// Called by the host when the server begins shutting down.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnServerStopping<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    guard("on_server_stopping", || {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_server_stopping(&JniServer);
    });
}

/// Returns mod-registered command names, one per line, so the host can wire them
/// into Brigadier.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeCommandNames<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let names = registry()
        .read()
        .expect("registry poisoned")
        .command_names()
        .join("\n");
    env.new_string(names)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Runs a registered command and returns its reply (empty string if none).
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnCommand<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    name: JString<'l>,
    args: JString<'l>,
    source: JString<'l>,
    uuid: JString<'l>,
) -> jstring {
    let ctx = CommandContext {
        name: env.get_string(&name).map(String::from).unwrap_or_default(),
        args: env.get_string(&args).map(String::from).unwrap_or_default(),
        source: env.get_string(&source).map(String::from).unwrap_or_default(),
        uuid: env.get_string(&uuid).map(String::from).unwrap_or_default(),
    };

    let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_command(&ctx, &JniServer)
            .unwrap_or_default()
    }))
    .unwrap_or_else(|_| {
        yog_logging::error!("a mod panicked handling command `{}` (ignored)", ctx.name);
        String::new()
    });

    env.new_string(reply)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Declared custom items as key=value lines, for the host to register.
///
/// Format per line: `id\tkey=value\t...` — always `id` first, then
/// tab-separated `key=value` pairs. Unknown keys are ignored by the host.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeItemDefs<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let s = registry()
        .read()
        .expect("registry poisoned")
        .items()
        .iter()
        .map(|d| {
            let mut parts = vec![d.id.clone()];
            parts.push(format!("max_stack={}", d.max_stack));
            if let Some(n) = &d.name    { parts.push(format!("name={n}")); }
            if let Some(t) = &d.tooltip { parts.push(format!("tooltip={t}")); }
            if d.max_damage > 0   { parts.push(format!("max_damage={}", d.max_damage)); }
            if d.fire_resistant   { parts.push("fire_resistant=1".into()); }
            if d.fuel_ticks > 0   { parts.push(format!("fuel_ticks={}", d.fuel_ticks)); }
            if let Some(f) = &d.food {
                parts.push(format!(
                    "food={}:{}:{}",
                    f.nutrition, f.saturation,
                    if f.can_always_eat { 1 } else { 0 }
                ));
            }
            parts.join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n");
    env.new_string(s)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// A packet arrived on the server from a client.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnPacket<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    channel: JString<'l>,
    player: JString<'l>,
    payload: JByteArray<'l>,
) {
    let event = PacketEvent {
        channel: env.get_string(&channel).map(String::from).unwrap_or_default(),
        player: env.get_string(&player).map(String::from).unwrap_or_default(),
        payload: env.convert_byte_array(&payload).unwrap_or_default(),
    };
    guard("on_packet", || {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_packet(&event, &JniServer);
    });
}

/// A packet arrived on the client from the server.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnClientPacket<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    channel: JString<'l>,
    payload: JByteArray<'l>,
) {
    let event = PacketEvent {
        channel: env.get_string(&channel).map(String::from).unwrap_or_default(),
        player: String::new(),
        payload: env.convert_byte_array(&payload).unwrap_or_default(),
    };
    guard("on_client_packet", || {
        registry()
            .read()
            .expect("registry poisoned")
            .dispatch_client_packet(&event, &JniServer);
    });
}

/// Server-receiver channels, one per line.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativePacketChannels<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let s = registry()
        .read()
        .expect("registry poisoned")
        .packet_channels()
        .join("\n");
    env.new_string(s)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Client-receiver channels, one per line.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeClientPacketChannels<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let s = registry()
        .read()
        .expect("registry poisoned")
        .client_packet_channels()
        .join("\n");
    env.new_string(s)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Declared custom blocks as key=value lines, for the host to register.
///
/// Format per line: `id\tkey=value\t...` — always `id` first, then
/// tab-separated `key=value` pairs. Unknown keys are ignored by the host.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeBlockDefs<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jstring {
    let s = registry()
        .read()
        .expect("registry poisoned")
        .blocks()
        .iter()
        .map(|d| {
            let mut parts = vec![d.id.clone()];
            parts.push(format!("hardness={}", d.hardness));
            parts.push(format!("resistance={}", d.resistance));
            if let Some(n) = &d.name { parts.push(format!("name={n}")); }
            if let Some(s) = d.shape {
                parts.push(format!("shape={}:{}:{}:{}:{}:{}", s[0], s[1], s[2], s[3], s[4], s[5]));
            }
            if d.light_level > 0  { parts.push(format!("light={}", d.light_level)); }
            if let Some(snd) = &d.sound { parts.push(format!("sound={snd}")); }
            if d.requires_tool    { parts.push("requires_tool=1".into()); }
            if d.no_collision     { parts.push("no_collision=1".into()); }
            if d.slipperiness > 0.0 { parts.push(format!("slipperiness={}", d.slipperiness)); }
            parts.join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n");
    env.new_string(s)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
