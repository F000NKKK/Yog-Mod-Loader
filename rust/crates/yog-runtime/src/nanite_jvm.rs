//! Reflection for nanites with a real JVM (`dev.yog.nanite.YogReflect`):
//! a constructor, a private method and field by name, a missing member,
//! a held object let go. Needs a JDK: `cargo test -p yog-runtime nanite_jvm -- --ignored`.

use std::path::{Path, PathBuf};

use jni::objects::JString;
use yog_event::nanite::{ClassMap, Mappings, MemberMap};

#[test]
#[ignore = "needs a JDK; run with --ignored"]
fn a_nanite_reflects_on_private_members() {
    let Some(jdk) = crate::test_jvm::jdk() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let classes = std::env::temp_dir().join(format!("yog-nanite-jvm-{}", std::process::id()));
    let common = root.join("../../../java-common/src/main/java/dev/yog");
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
        root.join("tests/java/dev/yog/nanite/ReflectProbe.java"),
    ])
    .collect();
    assert!(
        crate::test_jvm::javac(&jdk, &classes, &sources),
        "javac failed"
    );
    write_table(&classes);
    let vm = crate::test_jvm::start(&jdk, &classes);
    let mut env = vm.attach_current_thread().unwrap();
    let out = env
        .call_static_method(
            "dev/yog/nanite/ReflectProbe",
            "run",
            "()Ljava/lang/String;",
            &[],
        )
        .and_then(|v| v.l());
    if env.exception_check().unwrap_or(false) {
        env.exception_describe().ok();
    }
    let out: String = env.get_string(&JString::from(out.unwrap())).unwrap().into();
    assert_eq!(out, "ababab,7,42,true,1,0,true");
    let mapped = env
        .call_static_method(
            "dev/yog/nanite/ReflectProbe",
            "runMapped",
            "()Ljava/lang/String;",
            &[],
        )
        .and_then(|v| v.l())
        .unwrap();
    let mapped: String = env.get_string(&JString::from(mapped)).unwrap().into();
    assert_eq!(
        mapped, "xx,7",
        "named members found by their run-time names"
    );
}

/// A table renaming the probe: `NamedProbe` is `ReflectProbe` at run time,
/// its `double` `twice`, its `hidden` `secret`.
fn write_table(classes: &Path) {
    let member = |named: &str, desc: &str, runtime: &str| MemberMap {
        named: named.into(),
        desc: desc.into(),
        runtime: runtime.into(),
    };
    let table = Mappings {
        runtime: yog_ids::mapping_ns::INTERMEDIARY.into(),
        classes: vec![ClassMap {
            named: "dev/yog/nanite/NamedProbe".into(),
            runtime: "dev/yog/nanite/ReflectProbe".into(),
            fields: vec![member("hidden", "", "secret")],
            methods: vec![member(
                "double",
                "(Ljava/lang/String;I)Ljava/lang/String;",
                "twice",
            )],
        }],
    };
    std::fs::write(
        classes.join(yog_ids::mapping_file::NAME),
        yog_data::to_bytes(&table),
    )
    .unwrap();
}
