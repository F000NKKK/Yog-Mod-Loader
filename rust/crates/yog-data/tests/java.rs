//! The hosts' Java reader (`java-common/.../dev/yog/data/YogData.java`)
//! against this crate: Java decodes what Rust encodes and encodes it back
//! to the same bytes; truncated input fails cleanly. Needs a JDK, so it's
//! run on demand: `cargo test -p yog-data --test java -- --ignored`.

use std::path::{Path, PathBuf};
use std::process::Command;

use yog_data::{Map, Ref, Value, binary, text};

/// Gson, which YogData's JSON half uses: the newest Gradle cached.
fn gson() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let dir = home.join(".gradle/caches/modules-2/files-2.1/com.google.code.gson/gson");
    let mut jars: Vec<PathBuf> = walk(&dir)
        .into_iter()
        .filter(|p| {
            p.extension().is_some_and(|e| e == "jar") && !p.to_string_lossy().contains("sources")
        })
        .collect();
    jars.sort();
    jars.pop()
        .expect("gson in the Gradle cache (build a host once)")
}

/// Substrate Data's Java port, from its sibling checkout (as `java-api/substrate.gradle` has it).
const SUBSTRATE_JAVA: &str = "../../../../../Substrate-Data/ports/java/src/main/java";

/// Every `.java` under `dir`.
fn java_files(dir: &Path) -> Vec<PathBuf> {
    walk(dir)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "java"))
        .collect()
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .flat_map(|e| {
            if e.path().is_dir() {
                walk(&e.path())
            } else {
                vec![e.path()]
            }
        })
        .collect()
}

fn jdk_tool(name: &str) -> PathBuf {
    let jdk17 = Path::new("/usr/lib/jvm/java-17-openjdk-amd64/bin").join(name);
    if jdk17.exists() {
        jdk17
    } else {
        PathBuf::from(name)
    }
}

fn every_type() -> Value {
    let mut m = Map::new();
    m.insert(
        "ints",
        Value::List(
            [0, 1, -1, 63, -64, 64, i64::MAX, i64::MIN]
                .map(Value::Int)
                .to_vec(),
        ),
    );
    m.insert(
        "floats",
        Value::List(
            [0.5, -0.0, 1e300, f64::MIN_POSITIVE]
                .map(Value::Float)
                .to_vec(),
        ),
    );
    m.insert(
        "misc",
        Value::List(vec![
            Value::Null,
            Value::Bool(true),
            Value::Bool(false),
            Value::Str("ünï😀".into()),
        ]),
    );
    let r = |kind: &str, id: &str| Value::Ref(Ref::parse(kind, id).expect("a reference"));
    m.insert("ref", r("item", "minecraft:stick"));
    m.insert("tag", r("tag", "block:minecraft:logs"));
    m.insert("res", r("res", "Strings.a.b"));
    m.insert("text", Value::Str("{b|x}".into()));
    m.insert("image", r("image", "sprite:x"));
    m.insert("color", Value::Int(0x80FF_0000));
    m.insert("lang", r("lang", "gui.done"));
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
        .args(["--release", "17", "-cp"])
        .arg(gson())
        .arg("-d")
        .arg(&dir)
        .arg(root.join("../../../java-api/src/main/java/dev/yog/data/YogData.java"))
        // The words it spells typed values with.
        .arg(root.join("../../../java-api/src/main/java/dev/yog/YogNames.java"))
        // Substrate Data's Java port, which YogData reads and writes through.
        .args(java_files(&root.join(SUBSTRATE_JAVA)))
        .arg(root.join("tests/java/RoundTrip.java"))
        .status()
        .expect("javac");
    assert!(javac.success());

    let book = text::parse_value(include_str!("hexbook.ybook")).unwrap();
    for (name, value) in [("types", every_type()), ("book", book)] {
        let bytes = binary::encode(&value);
        let (input, output) = (
            dir.join(format!("{name}.in")),
            dir.join(format!("{name}.out")),
        );
        std::fs::write(&input, &bytes).unwrap();
        let run = Command::new(jdk_tool("java"))
            .arg("-cp")
            .arg(std::env::join_paths([dir.clone(), gson()]).unwrap())
            .arg("RoundTrip")
            .arg(&input)
            .arg(&output)
            .status()
            .expect("java");
        assert!(run.success(), "{name}: Java failed");
        assert!(
            std::fs::read(&output).unwrap() == bytes,
            "{name}: Java wrote different bytes"
        );
    }
    // A packed list of ints (what a schema of numbers writes) and a stream
    // read as lists: Java writes them back as plain lists, the same values.
    let stream: yog_data::Stream<String> = ["minecraft:stone", "x", "minecraft:stone"]
        .map(String::from)
        .into_iter()
        .collect();
    let lists = [
        ("packed", yog_data::to_bytes(&vec![1u16, 300, 7, u16::MAX])),
        ("stream", yog_data::to_bytes(&stream)),
        (
            "floats",
            yog_data::to_bytes(&vec![0.5f64, -1e300, f64::MIN_POSITIVE]),
        ),
        ("floats32", yog_data::to_bytes(&vec![0.25f32, -3.5])),
        ("float32", yog_data::to_bytes(&1.5f32)),
    ];
    for (name, bytes) in lists {
        let (input, output) = (
            dir.join(format!("{name}.in")),
            dir.join(format!("{name}.out")),
        );
        std::fs::write(&input, &bytes).unwrap();
        let run = Command::new(jdk_tool("java"))
            .arg("-cp")
            .arg(std::env::join_paths([dir.clone(), gson()]).unwrap())
            .arg("RoundTrip")
            .arg(&input)
            .arg(&output)
            .status()
            .expect("java");
        assert!(run.success(), "{name}: Java failed");
        assert_eq!(
            binary::decode(&std::fs::read(&output).unwrap()).unwrap(),
            binary::decode(&bytes).unwrap(),
            "{name}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
