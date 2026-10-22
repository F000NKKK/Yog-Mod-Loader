use super::*;

static VISITS: WorldData<u32> = WorldData::new("visits");
static COINS: PlayerData<i64> = PlayerData::new("coins");

#[test]
fn kept_values_are_saved_with_the_world_and_read_back_after_it_closes() {
    let root = std::env::temp_dir().join(format!("yog-api-storage-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    *WORLD.lock() = Some(root.clone());
    VISITS.with_mut(|n| *n += 2);
    COINS.with_mut("uuid-1", |c| *c = 7);
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
    let _ = std::fs::remove_dir_all(&root);
}
