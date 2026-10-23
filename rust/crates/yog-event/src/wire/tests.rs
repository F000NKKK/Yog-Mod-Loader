//! Events round-trip through their bytes; the Java events are current.

use yog_core::BlockPos;

use crate::*;

#[test]
fn an_event_round_trips_through_its_bytes() {
    let e = block::Break {
        player_name: "Steve".into(),
        block_id: "minecraft:stone".into(),
        pos: BlockPos { x: 1, y: -2, z: 3 },
        dimension: "minecraft:overworld".into(),
    };
    let back = block::Break::from_bytes(&e.to_bytes()).expect("reads back");
    assert_eq!(
        (back.player_name, back.block_id, back.pos, back.dimension),
        (e.player_name, e.block_id, e.pos, e.dimension)
    );
    assert_eq!(block::Break::ADDRESS, "yog:event/block/break");
    assert_eq!(block::Break::PRE, Some("yog:event/block/break/pre"));
}

#[test]
fn the_java_messages_are_current() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../java-common/src/main/java/dev/yog/bus");
    for (class, text) in java::files("dev.yog.bus") {
        let Ok(on_disk) = std::fs::read_to_string(dir.join(format!("{class}.java"))) else {
            return;
        };
        assert!(
            on_disk == text,
            "{}/{class}.java is stale: cargo run -p yog-event --bin yog-event-export -- {} dev.yog.bus",
            dir.display(),
            dir.display()
        );
    }
}

#[test]
fn a_keyed_event_names_its_key_field_and_branches_by_it() {
    assert_eq!(entity::game::Tick::KEY, Some("entity_type"));
    assert_eq!(block::Break::KEY, None);
    assert_eq!(
        keyed(entity::game::Tick::ADDRESS, "mymod:golem"),
        "yog:event/entity/game/tick/mymod:golem"
    );
}

#[test]
fn a_keyed_service_goes_to_its_keys_branch() {
    let insert = block::Insert {
        block_id: "mymod:crate".into(),
        pos: BlockPos { x: 0, y: 64, z: 0 },
        dimension: "minecraft:overworld".into(),
        item_id: "minecraft:stone".into(),
        count: 1,
        nbt: String::new(),
        simulate: true,
    };
    assert_eq!(insert.key(), Some("mymod:crate"));
    assert_eq!(insert.target(), "yog:block/insert/mymod:crate");
    assert!(
        services()
            .iter()
            .any(|s| s.domain == "block" && s.name == "Insert")
    );
}

/// A message's address is its target's atom: an unkeyed type's kept once,
/// a keyed one's its branch's, every time.
#[test]
fn addresses_are_their_targets_atoms() {
    let insert = |block: &str| block::Insert {
        block_id: block.into(),
        pos: BlockPos { x: 0, y: 64, z: 0 },
        dimension: "minecraft:overworld".into(),
        item_id: "minecraft:stone".into(),
        count: 1,
        nbt: String::new(),
        simulate: true,
    };
    for block in ["mymod:crate", "mymod:barrel"] {
        let i = insert(block);
        assert_eq!(i.address(), crate::resolve(&i.target()));
    }
    assert_ne!(
        insert("mymod:crate").address(),
        insert("mymod:barrel").address()
    );
    let give = crate::player::GiveItem {
        player: "Steve".into(),
        item: "minecraft:diamond".into(),
        count: 1,
    };
    assert_eq!(
        (give.address(), give.address()),
        (
            crate::resolve(&give.target()),
            crate::resolve(&give.target())
        )
    );
    assert_eq!(crate::resolve(""), crate::NO_ADDRESS);
}

/// With no bus reached, actions answer their replies' defaults — through a
/// function or a handle alike.
#[test]
fn actions_answer_defaults_without_a_bus() {
    use crate::{Player, World, entity, player};
    assert!(!player::give_item("Steve", "minecraft:diamond", 1u32));
    assert!(!Player::new("Steve").give_item("minecraft:diamond", 1u32));
    assert_eq!(Player::new("Steve").entity(), None);
    assert_eq!(entity::position("u"), None);
    assert_eq!(World::overworld().get_block(BlockPos::new(0, 0, 0)), None);
    assert!(
        World::overworld()
            .scan_region(BlockPos::new(0, 0, 0), BlockPos::new(1, 1, 1))
            .is_empty()
    );
}
