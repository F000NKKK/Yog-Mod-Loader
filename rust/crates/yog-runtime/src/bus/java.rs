//! The bus's Java side: `dev.yog.YogBus`'s natives (Java resolves, provides,
//! listens, calls and emits) and running a Java handler. A Java reply is the
//! status byte, then the reply's bytes.

use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JString, JValue};
use jni::sys::{jboolean, jbyteArray, jint};
use yog_abi::{YogStatus, status};

use super::{Handler, ROUTER};

/// `dev.yog.YogBus`, and its handler entry point.
const BUS_CLASS: &str = "dev/yog/YogBus";
const HANDLE: &str = "handle";
const HANDLE_SIGNATURE: &str = "(I[B)[B";
/// A Java reply's status byte before its bytes.
const STATUS_LEN: usize = 1;

/// Runs Java handler `handler`.
pub(super) fn run(handler: i32, payload: &[u8]) -> (YogStatus, Vec<u8>) {
    let Some(mut env) = crate::get_env() else {
        return (status::NO_HANDLER, Vec::new());
    };
    let reply = (|| {
        let bytes = env.byte_array_from_slice(payload).ok()?;
        let out = env
            .call_static_method(BUS_CLASS, HANDLE, HANDLE_SIGNATURE, &[JValue::Int(handler), JValue::Object(&bytes)])
            .ok()?
            .l()
            .ok()?;
        env.convert_byte_array(JByteArray::from(out)).ok()
    })();
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
        return (status::FAILED, Vec::new());
    }
    match reply {
        Some(r) if r.len() >= STATUS_LEN => (YogStatus::from(r[0]), r[STATUS_LEN..].to_vec()),
        _ => (status::FAILED, Vec::new()),
    }
}

/// `status` and `reply` as a Java reply.
fn java_reply(env: &mut JNIEnv, st: YogStatus, reply: &[u8]) -> jbyteArray {
    let mut out = Vec::with_capacity(STATUS_LEN + reply.len());
    out.push(u8::try_from(st).unwrap_or(u8::MAX));
    out.extend_from_slice(reply);
    env.byte_array_from_slice(&out).map_or(std::ptr::null_mut(), |a| a.into_raw())
}

fn bytes(env: &mut JNIEnv, payload: &JByteArray) -> Vec<u8> {
    env.convert_byte_array(payload).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeResolve<'l>(mut env: JNIEnv<'l>, _class: JClass<'l>, address: JString<'l>) -> jint {
    let name: String = env.get_string(&address).map(String::from).unwrap_or_default();
    ROUTER.resolve(&name) as jint
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeProvide<'l>(_env: JNIEnv<'l>, _class: JClass<'l>, address: jint, handler: jint) -> jboolean {
    ROUTER.provide(address as u32, Handler::Java(handler)) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeOn<'l>(_env: JNIEnv<'l>, _class: JClass<'l>, event: jint, priority: jint, handler: jint) {
    ROUTER.on(event as u32, priority, Handler::Java(handler));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeCall<'l>(mut env: JNIEnv<'l>, _class: JClass<'l>, to: jint, payload: JByteArray<'l>) -> jbyteArray {
    let payload = bytes(&mut env, &payload);
    let (st, reply) = ROUTER.call(to as u32, &payload, run);
    java_reply(&mut env, st, &reply)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeEmit<'l>(mut env: JNIEnv<'l>, _class: JClass<'l>, event: jint, payload: JByteArray<'l>) -> jbyteArray {
    let payload = bytes(&mut env, &payload);
    let (st, reply) = ROUTER.emit(event as u32, &payload, run);
    java_reply(&mut env, st, &reply)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_yog_YogBus_nativeHeard<'l>(_env: JNIEnv<'l>, _class: JClass<'l>, event: jint) -> jboolean {
    ROUTER.heard(event as u32) as jboolean
}
