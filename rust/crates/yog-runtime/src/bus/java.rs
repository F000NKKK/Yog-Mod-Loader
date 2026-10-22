//! The bus's Java side: `dev.yog.YogBus`'s natives (Java resolves, provides,
//! listens, calls and emits) and running a Java handler. A Java reply is the
//! status byte, then the reply's bytes.

use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jboolean, jbyteArray, jint, jobject};
use yog_abi::{YogReply, YogStatus, status};

use super::{Handler, ROUTER};
use crate::jcall::jcall;

/// A Java reply's status byte before its bytes (the way a call too big for
/// the arena goes).
const STATUS_LEN: usize = 1;

/// Runs Java handler `handler`: through this thread's arena, or — a payload
/// too big for it — as a `byte[]`.
pub(super) fn run(handler: i32, payload: &[u8], reply: &YogReply) -> YogStatus {
    let Some(mut env) = crate::jni_util::attached_env() else {
        return status::NO_HANDLER;
    };
    if let Some(st) = super::arena::run_java(&mut env, handler, payload, reply) {
        return st;
    }
    let answer: Option<Vec<u8>> = unsafe {
        jcall!(
            &mut env,
            "dev/yog/YogBus",
            "handle",
            [I(handler), Bytes(payload)]
        )
    };
    match answer {
        Some(r) if r.len() >= STATUS_LEN => {
            unsafe { YogReply::send(reply, &r[STATUS_LEN..]) };
            YogStatus::from(r[0])
        }
        _ => status::FAILED,
    }
}

/// `status` and `reply` as a Java reply.
fn java_reply(env: &mut JNIEnv, st: YogStatus, reply: &[u8]) -> jbyteArray {
    let mut out = Vec::with_capacity(STATUS_LEN + reply.len());
    out.push(u8::try_from(st).unwrap_or(u8::MAX));
    out.extend_from_slice(reply);
    env.byte_array_from_slice(&out)
        .map_or(std::ptr::null_mut(), |a| a.into_raw())
}

fn bytes(env: &mut JNIEnv, payload: &JByteArray) -> Vec<u8> {
    env.convert_byte_array(payload).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeResolve<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    address: JString<'l>,
) -> jint {
    crate::jni_util::guarded("nativeResolve", 0, || {
        let name: String = env
            .get_string(&address)
            .map(String::from)
            .unwrap_or_default();
        ROUTER.resolve(&name) as jint
    })
}

/// `name`'s atom (`YogAtoms`): the process's table, shared with every mod.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogAtoms_nativeAtom<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    name: JString<'l>,
) -> jint {
    crate::jni_util::guarded("nativeAtom", 0, || {
        let name: String = env.get_string(&name).map(String::from).unwrap_or_default();
        yog_data::atom::local::intern(&name) as jint
    })
}

/// Atom `n`'s name; null when there's none.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogAtoms_nativeAtomName<'l>(
    env: JNIEnv<'l>,
    _class: JClass<'l>,
    n: jint,
) -> jobject {
    crate::jni_util::guarded("nativeAtomName", std::ptr::null_mut(), || {
        let Some(name) = u32::try_from(n).ok().and_then(yog_data::atom::local::name) else {
            return std::ptr::null_mut();
        };
        env.new_string(name)
            .map_or(std::ptr::null_mut(), |s| s.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeProvide<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    address: jint,
    handler: jint,
) -> jboolean {
    crate::jni_util::guarded("nativeProvide", 0, || {
        ROUTER.provide(address as u32, Handler::Java(handler)) as jboolean
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeOn<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    event: jint,
    priority: jint,
    handler: jint,
) {
    crate::jni_util::guard("nativeOn", || {
        ROUTER.on(event as u32, priority, Handler::Java(handler));
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeCall<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    to: jint,
    payload: JByteArray<'l>,
) -> jbyteArray {
    crate::jni_util::guarded("nativeCall", std::ptr::null_mut(), || {
        let payload = bytes(&mut env, &payload);
        let (st, reply) = ROUTER.call_vec(to as u32, &payload, run);
        java_reply(&mut env, st, &reply)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeEmit<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    event: jint,
    payload: JByteArray<'l>,
) -> jbyteArray {
    crate::jni_util::guarded("nativeEmit", std::ptr::null_mut(), || {
        let payload = bytes(&mut env, &payload);
        let mut reply = Vec::new();
        let st = ROUTER.emit(event as u32, &payload, &YogReply::to_vec(&mut reply), run);
        java_reply(&mut env, st, &reply)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeHeard<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    event: jint,
) -> jboolean {
    crate::jni_util::guarded("nativeHeard", 0, || ROUTER.heard(event as u32) as jboolean)
}
