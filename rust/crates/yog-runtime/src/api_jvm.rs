//! The bus for Java mods with a real JVM (`dev.yog.api.YogBus`): a Java
//! record packs to the bytes the generated (Rust-shaped) record does; a
//! provider answers a call, a listener changes an event, a pre-listener
//! cancels one, a generated typed `on` hears its event — all through the
//! router. Needs a JDK: `cargo test -p yog-runtime api_jvm -- --ignored`.

use std::path::{Path, PathBuf};

use jni::JNIEnv;
use jni::objects::JString;

const PROBE: &str = "dev/yog/api/ApiProbe";

fn string(env: &mut JNIEnv, method: &str) -> String {
    let out = env
        .call_static_method(PROBE, method, "()Ljava/lang/String;", &[])
        .and_then(|v| v.l());
    if env.exception_check().unwrap_or(false) {
        env.exception_describe().ok();
    }
    env.get_string(&JString::from(out.unwrap())).unwrap().into()
}

#[test]
#[ignore = "needs a JDK; run with --ignored"]
fn java_mods_on_the_bus() {
    let Some(jvm) = crate::test_jvm_shared::shared() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let common = root.join("../../../java-common/src/main/java/dev/yog");
    let sources: Vec<PathBuf> = [
        "YogNames.java",
        "YogBus.java",
        "YogMessage.java",
        "YogAtoms.java",
        "bus/YogTypes.java",
        "bus/YogEvents.java",
        "data/YogJava.java",
        "data/YogData.java",
        "data/YogFrame.java",
        "util/YogNativeRuntime.java",
        "api/YogBus.java",
        "api/YogRecordFrames.java",
    ]
    .map(|f| common.join(f))
    .into_iter()
    .chain([
        root.join("tests/java/dev/yog/YogLog.java"),
        root.join("tests/java/dev/yog/YogPlatform.java"),
        root.join("tests/java/dev/yog/api/ApiProbe.java"),
    ])
    .collect();
    assert!(jvm.compile(&sources), "javac failed");
    let _ = crate::globals::JAVA_VM
        .set(unsafe { jni::JavaVM::from_raw(jvm.vm.get_java_vm_pointer()) }.unwrap());
    let mut env = jvm.vm.attach_current_thread().unwrap();
    crate::bus::jvm_support::register_natives(&mut env).unwrap();
    assert_eq!(
        string(&mut env, "sameFrame"),
        "true,true",
        "a record is the generated record's frame"
    );
    assert_eq!(string(&mut env, "roundTrip"), "42,2x!,true,false,101");
}
