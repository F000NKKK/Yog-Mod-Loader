//! The arena with a real JVM: the host's own `YogBus.java` (a stand-in
//! logger, a probe calling and answering), its natives bound to the
//! runtime's, calls both ways through the arena — nested, and too big for
//! it. Needs a JDK: `cargo test -p yog-runtime arena_jvm -- --ignored`.

use std::os::raw::c_void;
use std::path::{Path, PathBuf};

use jni::objects::{JByteArray, JValue};
use jni::{JNIEnv, NativeMethod};
use yog_abi::{YogBytes, YogReply, YogStatus, status};

use super::arena::{self, BYTES};
use super::{Handler, ROUTER, java};

/// `YogBus`'s natives: their names, signatures and the runtime's functions.
fn natives() -> Vec<NativeMethod> {
    let n = |name: &str, sig: &str, f: *mut c_void| NativeMethod { name: name.into(), sig: sig.into(), fn_ptr: f };
    vec![
        n("nativeResolve", "(Ljava/lang/String;)I", java::Java_dev_yog_YogBus_nativeResolve as *mut c_void),
        n("nativeCall", "(I[B)[B", java::Java_dev_yog_YogBus_nativeCall as *mut c_void),
        n("nativeEmit", "(I[B)[B", java::Java_dev_yog_YogBus_nativeEmit as *mut c_void),
        n("nativeProvide", "(II)Z", java::Java_dev_yog_YogBus_nativeProvide as *mut c_void),
        n("nativeOn", "(III)V", java::Java_dev_yog_YogBus_nativeOn as *mut c_void),
        n("nativeHeard", "(I)Z", java::Java_dev_yog_YogBus_nativeHeard as *mut c_void),
        n("nativeArena", "()Ljava/nio/ByteBuffer;", arena::Java_dev_yog_YogBus_nativeArena as *mut c_void),
        n("nativeCallAt", "(III)J", arena::Java_dev_yog_YogBus_nativeCallAt as *mut c_void),
        n("nativeEmitAt", "(III)J", arena::Java_dev_yog_YogBus_nativeEmitAt as *mut c_void),
        n("nativeTakeReply", "()[B", arena::Java_dev_yog_YogBus_nativeTakeReply as *mut c_void),
    ]
}

/// A Rust service: the payload twice.
unsafe extern "C" fn twice(_ud: *mut c_void, payload: YogBytes, reply: *const YogReply) -> YogStatus {
    let p = unsafe { payload.as_slice() };
    unsafe { YogReply::send(reply, &[p, p].concat()) };
    status::OK
}

/// A Rust service calling Java's echo on its payload — a nested frame.
unsafe extern "C" fn through_java(_ud: *mut c_void, payload: YogBytes, reply: *const YogReply) -> YogStatus {
    let (st, out) = ROUTER.call_vec(ROUTER.resolve("test:echo"), unsafe { payload.as_slice() }, java::run);
    unsafe { YogReply::send(reply, &out) };
    st
}

fn provide_native(address: &str, f: yog_abi::YogHandlerFn) {
    ROUTER.provide(ROUTER.resolve(address), Handler::Native { owner: None, ud: 0, f });
}

/// Java's `ArenaProbe.call(address, payload)`: the status, then the reply.
fn java_call(env: &mut JNIEnv, address: &str, payload: &[u8]) -> (YogStatus, Vec<u8>) {
    let a = env.new_string(address).unwrap();
    let p = env.byte_array_from_slice(payload).unwrap();
    let out = env
        .call_static_method("dev/yog/ArenaProbe", "call", "(Ljava/lang/String;[B)[B", &[JValue::Object(&a), JValue::Object(&p)])
        .unwrap()
        .l()
        .unwrap();
    let out = env.convert_byte_array(JByteArray::from(out)).unwrap();
    (YogStatus::from(out[0]), out[1..].to_vec())
}

fn java_void(env: &mut JNIEnv, method: &str, sig: &str, args: &[JValue]) {
    env.call_static_method("dev/yog/ArenaProbe", method, sig, args).unwrap();
}

fn reversed(p: &[u8]) -> Vec<u8> {
    p.iter().rev().copied().collect()
}

#[test]
#[ignore = "needs a JDK; run with --ignored"]
fn calls_cross_the_arena_both_ways() {
    let Some(jdk) = crate::test_jvm::jdk() else { return };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let classes = std::env::temp_dir().join(format!("yog-arena-jvm-{}", std::process::id()));
    let java_common = root.join("../../../java-common/src/main/java/dev/yog");
    let sources: Vec<PathBuf> = [java_common.join("YogBus.java"), java_common.join("YogNames.java")]
        .into_iter()
        .chain(["YogLog.java", "ArenaProbe.java"].map(|f| root.join("tests/java/dev/yog").join(f)))
        .collect();
    assert!(crate::test_jvm::javac(&jdk, &classes, &sources), "javac failed");
    let vm = crate::test_jvm::start(&jdk, &classes);
    let _ = crate::JAVA_VM.set(unsafe { jni::JavaVM::from_raw(vm.get_java_vm_pointer()) }.unwrap());
    let mut env = vm.attach_current_thread().unwrap();
    env.register_native_methods("dev/yog/YogBus", &natives()).unwrap();

    // Rust → Java: in the arena, and a payload too big for it.
    let name = env.new_string("test:echo").unwrap();
    java_void(&mut env, "provideEcho", "(Ljava/lang/String;)V", &[JValue::Object(&name)]);
    let echo = ROUTER.resolve("test:echo");
    assert_eq!(ROUTER.call_vec(echo, b"hello", java::run), (status::OK, b"olleh".to_vec()));
    let huge: Vec<u8> = (0..BYTES + 1000).map(|i| i as u8).collect();
    assert_eq!(ROUTER.call_vec(echo, &huge, java::run), (status::OK, reversed(&huge)));
    // A reply too big for the arena is kept aside and taken.
    let name = env.new_string("test:big").unwrap();
    java_void(&mut env, "provideBig", "(Ljava/lang/String;I)V", &[JValue::Object(&name), JValue::Int(BYTES as i32 * 2)]);
    let (st, big) = ROUTER.call_vec(ROUTER.resolve("test:big"), b"x", java::run);
    assert_eq!((st, big.len()), (status::OK, BYTES * 2));

    // Java → Rust, and Java → Rust → Java (a nested frame above the caller's).
    provide_native("test:twice", twice);
    provide_native("test:through_java", through_java);
    assert_eq!(java_call(&mut env, "test:twice", b"ab"), (status::OK, b"abab".to_vec()));
    assert_eq!(java_call(&mut env, "test:through_java", b"abc"), (status::OK, b"cba".to_vec()));
    assert_eq!(java_call(&mut env, "test:nobody", b"?").0, status::NO_HANDLER);
    let (st, out) = java_call(&mut env, "test:twice", &huge);
    assert_eq!((st, out.len()), (status::OK, huge.len() * 2));
    // Every frame was taken back.
    assert_eq!(arena::frames_top(), 0);
    let _ = std::fs::remove_dir_all(&classes);
}
