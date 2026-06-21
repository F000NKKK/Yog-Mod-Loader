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
use jni::sys::jint;
use jni::{JNIEnv, JavaVM};

use yog_api::{
    BlockBreakEvent, BlockPos, ChatEvent, PlayerJoinEvent, PlayerLeaveEvent, Registry, Server,
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

    println!("[yog] runtime initialised — the gate is open.");
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
