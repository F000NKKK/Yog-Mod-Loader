//! Events round-trip through their bytes; the Java events are current.

use yog_core::BlockPos;

use super::*;

#[test]
fn an_event_round_trips_through_its_bytes() {
    let e = block::Break { player_name: "Steve".into(), block_id: "minecraft:stone".into(), pos: BlockPos { x: 1, y: -2, z: 3 }, dimension: "minecraft:overworld".into() };
    let back = block::Break::from_bytes(&e.to_bytes()).expect("reads back");
    assert_eq!((back.player_name, back.block_id, back.pos, back.dimension), (e.player_name, e.block_id, e.pos, e.dimension));
    assert_eq!(block::Break::ADDRESS, "yog:event/block/break");
    assert_eq!(block::Break::PRE, Some("yog:event/block/break/pre"));
}

#[test]
fn the_java_events_are_current() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../java-common/src/main/java/dev/yog/bus");
    let Ok(on_disk) = std::fs::read_to_string(dir.join("YogEvents.java")) else {
        return;
    };
    assert!(on_disk == java::java("dev.yog.bus"), "{}/YogEvents.java is stale: cargo run -p yog-event --bin yog-event-export -- {} dev.yog.bus", dir.display(), dir.display());
}
