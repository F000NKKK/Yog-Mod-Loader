//! The hosts' Java reader (`java-common/.../dev/yog/data/YogData.java`)
//! against this crate: Java decodes what Rust encodes and encodes it back
//! to the same bytes; truncated input fails cleanly. Needs a JDK, so it's
//! run on demand: `cargo test -p yog-data --test java -- --ignored`.

use std::path::{Path, PathBuf};
use std::process::Command;

use yog_data::{Map, Ref, Res, Value, binary, text};

fn jdk_tool(name: &str) -> PathBuf {
    let jdk17 = Path::new("/usr/lib/jvm/java-17-openjdk-amd64/bin").join(name);
    if jdk17.exists() { jdk17 } else { PathBuf::from(name) }
}

fn every_type() -> Value {
    let mut m = Map::new();
    m.insert("ints", Value::List([0, 1, -1, 63, -64, 64, i64::MAX, i64::MIN].map(Value::Int).to_vec()));
    m.insert("floats", Value::List([0.5, -0.0, 1e300, f64::MIN_POSITIVE].map(Value::Float).to_vec()));
    m.insert("misc", Value::List(vec![Value::Null, Value::Bool(true), Value::Bool(false), Value::Str("ünï😀".into())]));
    m.insert("ref", Value::Ref(Ref::new("item", "minecraft:stick")));
    m.insert("tag", Value::Tag(Ref::new("block", "minecraft:logs")));
    m.insert("res", Value::Res(Res { set: "Strings".into(), key: "a.b".into() }));
    m.insert("text", Value::Text("{b|x}".into()));
    m.insert("image", Value::Image("sprite:x".into()));
    m.insert("color", Value::Color(0x80FF_0000));
    m.insert("again", Value::Str("ünï😀".into()));
    Value::Map(m)
}

#[test]
#[ignore = "needs a JDK; run with --ignored"]
fn java_reads_and_writes_the_same_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = std::env::temp_dir().join(format!("yog-data-java-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let javac = Command::new(jdk_tool("javac"))
        .args(["--release", "17", "-d"])
        .arg(&dir)
        .arg(root.join("../../../java-common/src/main/java/dev/yog/data/YogData.java"))
        .arg(root.join("tests/java/RoundTrip.java"))
        .status()
        .expect("javac");
    assert!(javac.success());

    let book = text::parse_value(include_str!("../benches/hexbook.ybook")).unwrap();
    for (name, value) in [("types", every_type()), ("book", book)] {
        let bytes = binary::encode(&value);
        let (input, output) = (dir.join(format!("{name}.in")), dir.join(format!("{name}.out")));
        std::fs::write(&input, &bytes).unwrap();
        let run = Command::new(jdk_tool("java")).arg("-cp").arg(&dir).arg("RoundTrip").arg(&input).arg(&output).status().expect("java");
        assert!(run.success(), "{name}: Java failed");
        assert!(std::fs::read(&output).unwrap() == bytes, "{name}: Java wrote different bytes");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
