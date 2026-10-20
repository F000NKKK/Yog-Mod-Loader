//! What the JVM test and the bus benchmark share: the natives a real
//! `YogBus.java` / `YogAtoms.java` bind to (a test binary isn't loaded as
//! the JNI library, so they're registered by hand), and the router reached
//! the way the host reaches it.

use std::os::raw::c_void;

use jni::{JNIEnv, NativeMethod};
use yog_abi::YogHandlerFn;
#[cfg(feature = "bench")]
use yog_abi::{YogReply, YogStatus};

use super::{Handler, ROUTER, arena, java};

fn native(name: &str, sig: &str, f: *mut c_void) -> NativeMethod {
    NativeMethod { name: name.into(), sig: sig.into(), fn_ptr: f }
}

/// Binds `YogBus`'s and `YogAtoms`' natives to the runtime's functions.
pub fn register_natives(env: &mut JNIEnv) -> jni::errors::Result<()> {
    let bus = [
        native("nativeResolve", "(Ljava/lang/String;)I", java::Java_dev_yog_YogBus_nativeResolve as *mut c_void),
        native("nativeCall", "(I[B)[B", java::Java_dev_yog_YogBus_nativeCall as *mut c_void),
        native("nativeEmit", "(I[B)[B", java::Java_dev_yog_YogBus_nativeEmit as *mut c_void),
        native("nativeProvide", "(II)Z", java::Java_dev_yog_YogBus_nativeProvide as *mut c_void),
        native("nativeOn", "(III)V", java::Java_dev_yog_YogBus_nativeOn as *mut c_void),
        native("nativeHeard", "(I)Z", java::Java_dev_yog_YogBus_nativeHeard as *mut c_void),
        native("nativeArena", "()Ljava/nio/ByteBuffer;", arena::Java_dev_yog_YogBus_nativeArena as *mut c_void),
        native("nativeCallAt", "(III)J", arena::Java_dev_yog_YogBus_nativeCallAt as *mut c_void),
        native("nativeEmitAt", "(III)J", arena::Java_dev_yog_YogBus_nativeEmitAt as *mut c_void),
        native("nativeTakeReply", "()[B", arena::Java_dev_yog_YogBus_nativeTakeReply as *mut c_void),
    ];
    env.register_native_methods("dev/yog/YogBus", &bus)?;
    let atoms = [
        native("nativeAtom", "(Ljava/lang/String;)I", java::Java_dev_yog_YogAtoms_nativeAtom as *mut c_void),
        native("nativeAtomName", "(I)Ljava/lang/String;", java::Java_dev_yog_YogAtoms_nativeAtomName as *mut c_void),
    ];
    env.register_native_methods("dev/yog/YogAtoms", &atoms)
}

#[cfg(feature = "bench")]
pub fn resolve(name: &str) -> u32 {
    ROUTER.resolve(name)
}

/// Calls `to` as a mod does — Java handlers through the arena — its reply into `reply`.
#[cfg(feature = "bench")]
pub fn call(to: u32, payload: &[u8], reply: &mut Vec<u8>) -> YogStatus {
    reply.clear();
    ROUTER.call(to, payload, &YogReply::to_vec(reply), java::run)
}

/// Makes the native `f` answer `address`.
pub fn provide_native(address: &str, f: YogHandlerFn) {
    ROUTER.provide(ROUTER.resolve(address), Handler::Native { owner: None, ud: 0, f });
}
