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

use jni::objects::{JClass, JString, JValue};
use jni::sys::{jint, jstring};
use jni::{JNIEnv, JavaVM};
use libloading::{Library, Symbol};

use yog_api::{
    BlockBreakEvent, BlockPos, ChatEvent, CommandContext, PlayerJoinEvent, PlayerLeaveEvent,
    Registry, Server, UseItemEvent, ABI_VERSION,
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
) -> jstring {
    let ctx = CommandContext {
        name: env.get_string(&name).map(String::from).unwrap_or_default(),
        args: env.get_string(&args).map(String::from).unwrap_or_default(),
        source: env.get_string(&source).map(String::from).unwrap_or_default(),
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

/// Declared custom items as `id<TAB>max_stack` lines, for the host to register.
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
            format!(
                "{}\t{}\t{}\t{}",
                d.id,
                d.max_stack,
                d.name.as_deref().unwrap_or(""),
                d.tooltip.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    env.new_string(s)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Declared custom blocks as `id<TAB>hardness<TAB>resistance` lines.
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
            let mut line = format!(
                "{}\t{}\t{}\t{}",
                d.id,
                d.hardness,
                d.resistance,
                d.name.as_deref().unwrap_or("")
            );
            if let Some(s) = d.shape {
                line.push_str(&format!(
                    "\t{}\t{}\t{}\t{}\t{}\t{}",
                    s[0], s[1], s[2], s[3], s[4], s[5]
                ));
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n");
    env.new_string(s)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
