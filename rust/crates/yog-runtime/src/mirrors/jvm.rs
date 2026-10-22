//! The host's side of mirrors with a real JVM: `YogColumn.java` writing a
//! column the runtime made, read back in place — the header's layout and
//! a grown column's new memory as Java sees them. Needs a JDK:
//! `cargo test -p yog-runtime mirrors::jvm -- --ignored`.

use std::os::raw::c_void;
use std::path::{Path, PathBuf};

use jni::objects::JValue;
use jni::{JNIEnv, NativeMethod};
use yog_ids::key_space;
use yog_mirror::{ColumnRef, FieldSpec, Kind, MirrorSpec, Rate, Store};

const PROBE: &str = "dev/yog/mirror/MirrorProbe";

fn write_pos(env: &mut JNIEnv, header: u64, row: u32, p: [f64; 3]) {
    let args = [
        JValue::Long(header as i64),
        JValue::Int(row as i32),
        JValue::Double(p[0]),
        JValue::Double(p[1]),
        JValue::Double(p[2]),
    ];
    env.call_static_method(PROBE, "writePos", "(JIDDD)V", &args)
        .unwrap();
}

#[test]
#[ignore = "needs a JDK; run with --ignored"]
fn java_writes_a_column_rust_reads() {
    let Some(jvm) = crate::test_jvm_shared::shared() else {
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mirror = root.join("../../../java-common/src/main/java/dev/yog/mirror");
    let sources: Vec<PathBuf> = ["YogColumn.java", "YogMirrorMemory.java"]
        .map(|f| mirror.join(f))
        .into_iter()
        .chain([root.join("tests/java/dev/yog/mirror/MirrorProbe.java")])
        .collect();
    assert!(jvm.compile(&sources), "javac failed");
    let vm = &jvm.vm;
    let mut env = vm.attach_current_thread().unwrap();
    let buffer = NativeMethod {
        name: "nativeBuffer".into(),
        sig: "(JJ)Ljava/nio/ByteBuffer;".into(),
        fn_ptr: super::Java_dev_yog_mirror_YogMirrorMemory_nativeBuffer as *mut c_void,
    };
    env.register_native_methods("dev/yog/mirror/YogMirrorMemory", &[buffer])
        .unwrap();

    let mut store = Store::default();
    let entities = store.space(key_space::ENTITY, None).unwrap();
    let spec = MirrorSpec {
        name: "test:mirror/jvm".into(),
        space: key_space::ENTITY.into(),
        fields: vec![FieldSpec {
            name: "pos".into(),
            fact: None,
            kind: Kind::F64.code(),
            count: 3,
        }],
        clock: yog_mirror::Clock::Server,
    };
    let d = store.declare(&spec, 1, Rate::Every(1), None).unwrap();
    let column = d.fields[0].unwrap().column;
    let pos = unsafe { ColumnRef::new(column) }.unwrap();
    let first = store.insert(entities, 7, None);
    write_pos(&mut env, column as u64, first, [1.0, 2.0, 3.0]);
    assert_eq!(pos.get::<[f64; 3]>(first), Some([1.0, 2.0, 3.0]));

    // The column grows: Java writes its new memory, the old row kept.
    for k in 100..5_000 {
        store.insert(entities, k, None);
    }
    let last = store.row_of(entities, 4_999).unwrap();
    write_pos(&mut env, column as u64, last, [-4.0, 64.0, 0.5]);
    assert_eq!(pos.get::<[f64; 3]>(last), Some([-4.0, 64.0, 0.5]));
    assert_eq!(pos.get::<[f64; 3]>(first), Some([1.0, 2.0, 3.0]));

    // A bit column: the mirror's rows.
    let args = [JValue::Long(d.rows as i64), JValue::Int(last as i32)];
    env.call_static_method(PROBE, "setBit", "(JI)V", &args)
        .unwrap();
    assert!(unsafe { ColumnRef::new(d.rows) }.unwrap().bit(last));
}
