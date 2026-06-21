//! Yog runtime — the native library loaded by the Fabric host.
//!
//! It exposes JNI entry points that the Java side calls, translates the incoming
//! data into [`yog_api`] events, and dispatches them to registered Rust mods.
//!
//! Symbol naming follows the JNI convention `Java_<package>_<class>_<method>`,
//! here `dev.yog.NativeBridge`.

use std::sync::{OnceLock, RwLock};

use jni::objects::{JClass, JString};
use jni::sys::jint;
use jni::JNIEnv;

use yog_api::{BlockBreakEvent, BlockPos, ChatEvent, Registry};

/// Global registry of mod event handlers, initialised once on startup.
static REGISTRY: OnceLock<RwLock<Registry>> = OnceLock::new();

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

/// Called once by the Java host after the native library is loaded.
#[no_mangle]
pub extern "system" fn Java_dev_yog_NativeBridge_nativeInit<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) {
    let mut reg = registry().write().expect("registry poisoned");

    // MVP: mods are linked in. Roadmap (stage 3): load `.so` mods from a dir.
    yog_example_mod::register(&mut reg);

    println!("[yog] runtime initialised — the gate is open.");
}

/// Called by the host's Mixin when a player breaks a block (server side).
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
        .dispatch_block_break(&event);
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
        .dispatch_chat(&event);
}
