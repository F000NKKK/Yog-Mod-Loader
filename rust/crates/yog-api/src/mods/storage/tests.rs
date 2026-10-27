use super::*;

static VISITS: WorldData<u32> = WorldData::new("visits");
static COINS: PlayerData<i64> = PlayerData::new("coins");
static HITS: EntityData<u32> = EntityData::new("hits");
static GLOW: BlockData<u8> = BlockData::new("glow");

static SHOWN: PlayerData<u32> = PlayerData::synced("shown");
static WEATHER: WorldData<u8> = WorldData::synced("weather");

const DIM: &str = "minecraft:overworld";
const LAMP: yog_core::BlockPos = yog_core::BlockPos {
    x: -17,
    y: 64,
    z: 33,
};

#[test]
fn kept_values_are_saved_with_the_world_and_read_back_after_it_closes() {
    let _turn = WORLD_TESTS.lock();
    let root = std::env::temp_dir().join(format!("yog-api-storage-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    *WORLD.lock() = Some(root.clone());
    VISITS.with_mut(|n| *n += 2);
    COINS.with_mut("uuid-1", |c| *c = 7);
    HITS.with_mut("golem", |n| *n = 3);
    HITS.with_mut("zombie", |n| *n = 1);
    GLOW.with_mut((DIM, LAMP), |g| *g = 15);
    kept::gone(&Gone::Entity("zombie"));
    // The server stopping: its save is the world's last.
    CLOSING.store(true, Ordering::Release);
    saving();
    assert!(WORLD.lock().is_none(), "the world closed");
    assert_eq!(VISITS.with(|n| *n), 0, "no world open: nothing read");
    *WORLD.lock() = Some(root.clone());
    kept::forget_world();
    assert_eq!(VISITS.with(|n| *n), 2, "read back from its store");
    assert_eq!(COINS.with("uuid-1", |c| *c), 7);
    assert_eq!(COINS.with("uuid-2", |c| *c), 0);
    assert_eq!(HITS.with("golem", |n| *n), 3);
    assert_eq!(HITS.with("zombie", |n| *n), 0, "gone as it died");
    assert_eq!(
        GLOW.with((DIM, LAMP), |g| *g),
        15,
        "read back from its chunk's store"
    );
    kept::gone(&Gone::Block(DIM, LAMP));
    assert_eq!(GLOW.with((DIM, LAMP), |g| *g), 0, "gone as it broke");
    let _ = std::fs::remove_dir_all(&root);
}

/// What `every` (or the changed) synced values send now: owner, player, key, value.
fn sent(every: bool) -> Vec<(String, Option<String>, &'static str, Vec<u8>)> {
    let mut out = Vec::new();
    kept::sync(every, &mut |o| out.push((o.owner, o.to, o.key, o.bytes)));
    out.sort();
    out
}

#[test]
fn synced_values_go_to_who_sees_them_and_are_taken_in() {
    let _turn = WORLD_TESTS.lock();
    SHOWN.with_mut("uuid-1", |n| *n = 5);
    WEATHER.with_mut(|w| *w = 2);
    let changed = sent(false);
    assert_eq!(
        changed,
        [
            (String::new(), None, "weather", yog_data::to_bytes(&2u8)),
            (
                "uuid-1".into(),
                Some("uuid-1".into()),
                "shown",
                yog_data::to_bytes(&5u32)
            ),
        ]
    );
    assert!(sent(false).is_empty(), "sent once");
    assert_eq!(sent(true).len(), 2, "a joining player gets every one");
    // A client: what arrives is its read.
    assert!(kept::take("shown", "uuid-1", &yog_data::to_bytes(&8u32)));
    assert_eq!(SHOWN.with("uuid-1", |n| *n), 8);
    assert!(
        !kept::take("shown", "uuid-2", &yog_data::to_bytes(&3u32)),
        "not used yet"
    );
    kept::synced::arrived(yog_event::storage::SyncedRow {
        kind: crate::mod_id().to_owned(),
        owner: "uuid-2".into(),
        key: "shown".into(),
        bytes: yog_data::to_bytes(&3u32),
    });
    assert_eq!(
        SHOWN.with("uuid-2", |n| *n),
        3,
        "what arrived before it was used"
    );
}
