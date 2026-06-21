//! Yog runtime — the native library loaded by the Fabric host.
//!
//! It exposes JNI entry points that the Java side calls, translates the incoming
//! data into [`yog_api`] events, and dispatches them to registered Rust mods.
//! It also implements [`yog_api::Server`], the Rust → Minecraft path, by calling
//! back into the Java host through a cached [`JavaVM`].
//!
//! Symbol naming follows the JNI convention `Java_<package>_<class>_<method>`,
//! here `dev.yog.NativeBridge`.

use std::sync::{OnceLock, RwLock};

use jni::objects::{JClass, JString, JValue};
use jni::sys::{jint, jstring};
use jni::{JNIEnv, JavaVM};

use yog_api::{
    BlockBreakEvent, BlockPos, ChatEvent, CommandContext, PlayerJoinEvent, PlayerLeaveEvent,
    Registry, Server,
};

/// Global registry of mod event handlers, initialised once on startup.
static REGISTRY: OnceLock<RwLock<Registry>> = OnceLock::new();

/// Cached VM handle so we can call back into Java from any thread.
static JAVA_VM: OnceLock<JavaVM> = OnceLock::new();

fn registry() -> &'static RwLock<Registry> {
    REGISTRY.get_or_init(|| RwLock::new(Registry::default()))
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
}

/// Called once by the Java host after the native library is loaded.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeInit<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    if let Ok(vm) = env.get_java_vm() {
        let _ = JAVA_VM.set(vm);
    }

    let mut reg = registry().write().expect("registry poisoned");

    // MVP: mods are linked in. Roadmap (stage 3): load `.so` mods from a dir.
    yog_example_mod::register(&mut reg);

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

    registry()
        .read()
        .expect("registry poisoned")
        .dispatch_block_break(&event, &JniServer);
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

    registry()
        .read()
        .expect("registry poisoned")
        .dispatch_chat(&event, &JniServer);
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

    registry()
        .read()
        .expect("registry poisoned")
        .dispatch_player_join(&event, &JniServer);
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

    registry()
        .read()
        .expect("registry poisoned")
        .dispatch_player_leave(&event, &JniServer);
}

/// Called by the host once the server has finished starting.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnServerStarted<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    registry()
        .read()
        .expect("registry poisoned")
        .dispatch_server_started(&JniServer);
}

/// Called by the host when the server begins shutting down.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeOnServerStopping<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    registry()
        .read()
        .expect("registry poisoned")
        .dispatch_server_stopping(&JniServer);
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

    let reply = registry()
        .read()
        .expect("registry poisoned")
        .dispatch_command(&ctx, &JniServer)
        .unwrap_or_default();

    env.new_string(reply)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
