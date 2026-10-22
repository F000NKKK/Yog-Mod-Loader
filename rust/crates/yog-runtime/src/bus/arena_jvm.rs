//! The arena with a real JVM: the host's own `YogBus.java` (a stand-in
//! logger, a probe calling and answering), its natives bound to the
//! runtime's, calls both ways through the arena — nested, and too big for
//! it. Needs a JDK: `cargo test -p yog-runtime arena_jvm -- --ignored`.

use std::os::raw::c_void;
use std::path::{Path, PathBuf};

use jni::JNIEnv;
use jni::objects::{JByteArray, JValue};
use yog_abi::{YogBytes, YogReply, YogStatus, status};
use yog_data::Schema;
use yog_data::frame::{read_frame, to_frame};

use super::arena::{self, BYTES};
use super::jvm_support::provide_native;
use super::{ROUTER, java};

/// A Rust service: the payload twice.
unsafe extern "C" fn twice(
    _ud: *mut c_void,
    payload: YogBytes,
    reply: *const YogReply,
) -> YogStatus {
    let p = unsafe { payload.as_slice() };
    unsafe { YogReply::send(reply, &[p, p].concat()) };
    status::OK
}

/// A Rust service calling Java's echo on its payload — a nested frame.
unsafe extern "C" fn through_java(
    _ud: *mut c_void,
    payload: YogBytes,
    reply: *const YogReply,
) -> YogStatus {
    let (st, out) = ROUTER.call_vec(
        ROUTER.resolve("test:echo"),
        unsafe { payload.as_slice() },
        java::run,
    );
    unsafe { YogReply::send(reply, &out) };
    st
}

/// What the frame probes send: `ArenaProbe`'s {name, n}.
#[derive(Schema, Debug, PartialEq)]
struct Greet {
    name: String,
    n: i64,
}

/// A Rust service reading a frame: {"hey " + name, n + 1}.
unsafe extern "C" fn greet_back(
    _ud: *mut c_void,
    payload: YogBytes,
    reply: *const YogReply,
) -> YogStatus {
    let Some(g) = read_frame::<Greet>(unsafe { payload.as_slice() }) else {
        return status::BAD_PAYLOAD;
    };
    unsafe {
        YogReply::send(
            reply,
            &to_frame(&Greet {
                name: format!("hey {}", g.name),
                n: g.n + 1,
            }),
        )
    };
    status::OK
}

/// Java's `ArenaProbe.callGreet(address, name, n)`.
fn java_greet(env: &mut JNIEnv, address: &str, name: &str, n: i64) -> String {
    let a = env.new_string(address).unwrap();
    let who = env.new_string(name).unwrap();
    let out = env
        .call_static_method(
            "dev/yog/ArenaProbe",
            "callGreet",
            "(Ljava/lang/String;Ljava/lang/String;J)Ljava/lang/String;",
            &[JValue::Object(&a), JValue::Object(&who), JValue::Long(n)],
        )
        .unwrap()
        .l()
        .unwrap();
    env.get_string(&out.into()).unwrap().into()
}

/// Java's `ArenaProbe.call(address, payload)`: the status, then the reply.
fn java_call(env: &mut JNIEnv, address: &str, payload: &[u8]) -> (YogStatus, Vec<u8>) {
    let a = env.new_string(address).unwrap();
    let p = env.byte_array_from_slice(payload).unwrap();
    let out = env
        .call_static_method(
            "dev/yog/ArenaProbe",
            "call",
            "(Ljava/lang/String;[B)[B",
            &[JValue::Object(&a), JValue::Object(&p)],
        )
        .unwrap()
        .l()
        .unwrap();
    let out = env.convert_byte_array(JByteArray::from(out)).unwrap();
    (YogStatus::from(out[0]), out[1..].to_vec())
}

fn java_void(env: &mut JNIEnv, method: &str, sig: &str, args: &[JValue]) {
    env.call_static_method("dev/yog/ArenaProbe", method, sig, args)
        .unwrap();
}

fn reversed(p: &[u8]) -> Vec<u8> {
    p.iter().rev().copied().collect()
}

#[test]
#[ignore = "needs a JDK; run with --ignored"]
fn calls_cross_the_arena_both_ways() {
    let Some(jvm) = crate::test_jvm_shared::shared() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let java_common = root.join("../../../java-common/src/main/java/dev/yog");
    let sources: Vec<PathBuf> = [
        "YogBus.java",
        "YogMessage.java",
        "YogAtoms.java",
        "YogNames.java",
        "data/YogFrame.java",
        "data/YogData.java",
    ]
    .map(|f| java_common.join(f))
    .into_iter()
    .chain(["YogLog.java", "ArenaProbe.java"].map(|f| root.join("tests/java/dev/yog").join(f)))
    .collect();
    assert!(jvm.compile(&sources), "javac failed");
    let vm = &jvm.vm;
    let _ = crate::globals::JAVA_VM
        .set(unsafe { jni::JavaVM::from_raw(vm.get_java_vm_pointer()) }.unwrap());
    let mut env = vm.attach_current_thread().unwrap();
    super::jvm_support::register_natives(&mut env).unwrap();

    // Rust → Java: in the arena, and a payload too big for it.
    let name = env.new_string("test:echo").unwrap();
    java_void(
        &mut env,
        "provideEcho",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&name)],
    );
    let echo = ROUTER.resolve("test:echo");
    assert_eq!(
        ROUTER.call_vec(echo, b"hello", java::run),
        (status::OK, b"olleh".to_vec())
    );
    let huge: Vec<u8> = (0..BYTES + 1000).map(|i| i as u8).collect();
    assert_eq!(
        ROUTER.call_vec(echo, &huge, java::run),
        (status::OK, reversed(&huge))
    );
    // A reply too big for the arena is kept aside and taken.
    let name = env.new_string("test:big").unwrap();
    java_void(
        &mut env,
        "provideBig",
        "(Ljava/lang/String;I)V",
        &[JValue::Object(&name), JValue::Int(BYTES as i32 * 2)],
    );
    let (st, big) = ROUTER.call_vec(ROUTER.resolve("test:big"), b"x", java::run);
    assert_eq!((st, big.len()), (status::OK, BYTES * 2));

    // Java → Rust, and Java → Rust → Java (a nested frame above the caller's).
    provide_native("test:twice", twice);
    provide_native("test:through_java", through_java);
    assert_eq!(
        java_call(&mut env, "test:twice", b"ab"),
        (status::OK, b"abab".to_vec())
    );
    assert_eq!(
        java_call(&mut env, "test:through_java", b"abc"),
        (status::OK, b"cba".to_vec())
    );
    assert_eq!(
        java_call(&mut env, "test:nobody", b"?").0,
        status::NO_HANDLER
    );
    let (st, out) = java_call(&mut env, "test:twice", &huge);
    assert_eq!((st, out.len()), (status::OK, huge.len() * 2));

    // Frames packed and read in place: Rust → Java, Java → Rust, Java → Java.
    let name = env.new_string("test:greet").unwrap();
    java_void(
        &mut env,
        "provideGreet",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&name)],
    );
    let (st, out) = ROUTER.call_vec(
        ROUTER.resolve("test:greet"),
        &to_frame(&Greet {
            name: "steve".into(),
            n: 21,
        }),
        java::run,
    );
    assert_eq!(
        (st, read_frame::<Greet>(&out)),
        (
            status::OK,
            Some(Greet {
                name: "hi steve".into(),
                n: 42
            })
        )
    );
    provide_native("test:greet_back", greet_back);
    assert_eq!(
        java_greet(&mut env, "test:greet_back", "alex", 1),
        "0:hey alex:2"
    );
    assert_eq!(java_greet(&mut env, "test:greet", "bob", 5), "0:hi bob:10");
    // A frame that isn't one: the handler fails, and nothing is answered.
    assert_eq!(
        ROUTER.call_vec(ROUTER.resolve("test:greet"), b"", java::run),
        (status::FAILED, Vec::new())
    );
    // Atoms: Java and Rust number a name the same, whoever saw it first.
    let rust_first = yog_data::Atom::new("test:rust_first");
    let a = env.new_string("test:rust_first").unwrap();
    let java_n = env
        .call_static_method(
            "dev/yog/ArenaProbe",
            "atom",
            "(Ljava/lang/String;)I",
            &[JValue::Object(&a)],
        )
        .unwrap()
        .i()
        .unwrap();
    assert_eq!(java_n as u32, rust_first.number());
    let b = env.new_string("test:java_first").unwrap();
    let java_n = env
        .call_static_method(
            "dev/yog/ArenaProbe",
            "atom",
            "(Ljava/lang/String;)I",
            &[JValue::Object(&b)],
        )
        .unwrap()
        .i()
        .unwrap();
    assert_eq!(
        yog_data::Atom::new("test:java_first").number(),
        java_n as u32
    );
    let name = env
        .call_static_method(
            "dev/yog/ArenaProbe",
            "atomName",
            "(I)Ljava/lang/String;",
            &[JValue::Int(java_n)],
        )
        .unwrap()
        .l()
        .unwrap();
    assert_eq!(
        String::from(env.get_string(&name.into()).unwrap()),
        "test:java_first"
    );
    // Every frame was taken back.
    assert_eq!(arena::frames_top(), 0);
}
