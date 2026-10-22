//! A nanite's call, Java's side (`YogReflect.call`: a member resolved once,
//! called by its handle) against a `jvm/call` chain (`YogJava.invoke`:
//! parsed and looked up each time) and a plain Java call. The bus's own
//! crossing is `bus_calls`'. Needs a JDK:
//! `cargo bench -p yog-runtime --features bench --bench nanite`.

use std::path::{Path, PathBuf};

use jni::objects::JString;

#[path = "../src/test_jvm.rs"]
mod test_jvm;

fn main() {
    let Some(jdk) = test_jvm::jdk() else {
        eprintln!("no JDK: nothing measured");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let common = root.join("../../../java-common/src/main/java/dev/yog");
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("nanite-bench-classes");
    let sources: Vec<PathBuf> = [
        "nanite/YogReflect.java",
        "nanite/YogObjects.java",
        "nanite/YogMappings.java",
        "data/YogJava.java",
        "data/YogData.java",
        "data/YogFrame.java",
        "YogNames.java",
        "YogBus.java",
        "YogMessage.java",
        "YogAtoms.java",
        "bus/YogTypes.java",
    ]
    .map(|f| common.join(f))
    .into_iter()
    .chain([
        root.join("tests/java/dev/yog/YogLog.java"),
        root.join("benches/java/dev/yog/nanite/NaniteBench.java"),
    ])
    .collect();
    assert!(test_jvm::javac(&jdk, &out, &sources), "javac failed");
    let vm = test_jvm::start(&jdk, &out);
    let mut env = vm.attach_current_thread().expect("a thread on the JVM");
    let result = env
        .call_static_method(
            "dev/yog/nanite/NaniteBench",
            "run",
            "()Ljava/lang/String;",
            &[],
        )
        .and_then(|v| v.l())
        .expect("the bench runs");
    let result: String = env
        .get_string(&JString::from(result))
        .expect("its result")
        .into();
    let [plain, nanite, chain] = result.split(',').collect::<Vec<_>>()[..] else {
        panic!("unexpected result {result}");
    };
    println!("plain Java call   {plain} ns");
    println!("nanite (by handle) {nanite} ns");
    println!("jvm/call chain    {chain} ns");
}
