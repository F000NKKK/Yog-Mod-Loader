//! The hosts' generated records (`dev.yog.bus.YogTypes`) read the frames
//! Rust writes and write the same values back — run on demand, it needs a
//! JDK and Gson from the Gradle cache: `cargo test -p yog-core --test java_frames -- --ignored`.

use std::path::{Path, PathBuf};
use std::process::Command;

use yog_core::{BlockArea, BlockHit, BlockKind, BlockPos, EntityInfo, Face, LocalState, PlayerInfo, Point, ScreenSlot, Stack};
use yog_data::Schema;
use yog_data::frame::{read_frame, to_frame};

fn jdk_tool(name: &str) -> PathBuf {
    Path::new("/usr/lib/jvm/java-17-openjdk-amd64/bin").join(name)
}

fn gson() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let mut jars: Vec<PathBuf> = walk(&home.join(".gradle/caches/modules-2/files-2.1/com.google.code.gson/gson"))
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "jar") && !p.to_string_lossy().contains("sources"))
        .collect();
    jars.sort();
    jars.pop().expect("gson in the Gradle cache (build a host once)")
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries.flatten().flat_map(|e| if e.path().is_dir() { walk(&e.path()) } else { vec![e.path()] }).collect()
}

/// `value` through Java and back, as Java's record `record`.
fn through_java<T: Schema + PartialEq + std::fmt::Debug>(dir: &Path, record: &str, value: &T) {
    let (input, output) = (dir.join(format!("{record}.in")), dir.join(format!("{record}.out")));
    std::fs::write(&input, to_frame(value)).unwrap();
    let run = Command::new(jdk_tool("java"))
        .arg("-cp")
        .arg(std::env::join_paths([dir.to_path_buf(), gson()]).unwrap())
        .args(["FrameTrip", record])
        .arg(&input)
        .arg(&output)
        .status()
        .expect("java");
    assert!(run.success(), "{record}: Java failed");
    assert_eq!(read_frame::<T>(&std::fs::read(&output).unwrap()).as_ref(), Some(value), "{record}");
}

#[test]
#[ignore = "needs a JDK; run with --ignored"]
fn java_records_read_and_write_rust_frames() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let java = root.join("../../../java-common/src/main/java/dev/yog");
    let dir = std::env::temp_dir().join(format!("yog-frames-java-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let javac = Command::new(jdk_tool("javac"))
        .args(["--release", "17", "-cp"])
        .arg(gson())
        .arg("-d")
        .arg(&dir)
        .args(["YogNames.java", "data/YogData.java", "data/YogFrame.java", "bus/YogTypes.java"].map(|f| java.join(f)))
        .arg(root.join("tests/java/FrameTrip.java"))
        .status()
        .expect("javac");
    assert!(javac.success());

    let point = Point { x: 1.5, y: -64.0, z: 1e9 };
    through_java(&dir, "EntityInfo", &EntityInfo {
        type_id: "minecraft:zombie".into(),
        dimension: "minecraft:overworld".into(),
        pos: point,
        eye_height: 1.74,
        width: 0.6,
        height: 1.95,
        flags: u32::MAX,
        name: "Zom\u{1F600}bie".into(),
    });
    through_java(&dir, "BlockHit", &BlockHit { pos: BlockPos::new(-1, 2, 3), face: Face::West, hit: point });
    through_java(&dir, "LocalState", &LocalState {
        pos: point,
        yaw: 90.5,
        pitch: -12.25,
        velocity: point,
        on_ground: true,
        in_water: false,
        eye_height: 1.62,
        health: 19.5,
        food: 20,
        dimension: "minecraft:the_nether".into(),
        slot: 8,
        attack_strength: 0.75,
    });
    through_java(&dir, "ScreenSlot", &ScreenSlot { stack: Some(Stack { item: "minecraft:stick".into(), count: 64, snbt: "{}".into() }), x: -8, y: 140 });
    through_java(&dir, "ScreenSlot", &ScreenSlot { stack: None, x: 0, y: 0 });
    through_java(&dir, "PlayerInfo", &PlayerInfo {
        uuid: "u".into(),
        gamemode: "creative".into(),
        selected_slot: 4,
        xp_level: -3,
        xp_total: 1395,
        air: 300,
        may_fly: true,
        flying: false,
    });
    let kind = |id: &str| BlockKind { id: id.into(), solid: true, liquid: false, climbable: false, harmful: true };
    through_java(&dir, "BlockArea", &BlockArea {
        min: BlockPos::new(0, -64, 0),
        max: BlockPos::new(15, 319, 15),
        palette: vec![kind("minecraft:stone"), kind("minecraft:lava")],
        blocks: (0..4096).map(|i| (i % 2) as u16).collect(),
    });
    let _ = std::fs::remove_dir_all(&dir);
}
