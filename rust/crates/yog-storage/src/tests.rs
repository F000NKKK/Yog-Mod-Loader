use std::path::{Path, PathBuf};

use yog_data::Schema;

use super::*;

#[derive(Debug, Clone, PartialEq, Default, Schema)]
struct Board {
    tier: u8,
    chips: Vec<String>,
}

/// A folder of its own for one test.
fn root(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("yog-storage-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn board() -> Board {
    Board {
        tier: 2,
        chips: vec!["adder".into(), "tab\tand\nnewline".into()],
    }
}

/// The store as it is on disk: opened afresh once its root is closed.
fn reopened(root: &Path, scope: Scope<'_>) -> Store {
    close_under(root).unwrap();
    Store::open(root, "mymod", scope)
}

#[test]
fn a_typed_value_comes_back_after_the_world_closes() {
    let dir = root("roundtrip");
    let store = Store::open(&dir, "mymod", Scope::World);
    store.set("alu", &board());
    store.set("count", &3u32);
    assert_eq!(store.get::<Board>("alu"), Some(board()), "kept in memory");
    let store = reopened(&dir, Scope::World);
    assert_eq!(store.get::<Board>("alu"), Some(board()), "written on close");
    assert_eq!(store.get::<u32>("count"), Some(3));
    assert_eq!(store.get::<String>("count"), None, "not of that type");
    assert_eq!(store.keys(), ["alu", "count"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scopes_keep_apart_and_handles_share() {
    let dir = root("scopes");
    let one = Store::open(&dir, "mymod", Scope::Player("uuid-1"));
    Store::open(&dir, "mymod", Scope::Player("uuid-1")).set("coins", &7i64);
    assert_eq!(one.get::<i64>("coins"), Some(7), "one store, two handles");
    Store::open(&dir, "mymod", Scope::Chunk("minecraft:overworld", -1, 2))
        .set("ore", &"copper".to_owned());
    assert_eq!(
        Store::open(&dir, "mymod", Scope::Player("uuid-2")).get::<i64>("coins"),
        None
    );
    assert!(Store::open(&dir, "mymod", Scope::World).keys().is_empty());
    assert_eq!(
        reopened(&dir, Scope::Chunk("minecraft:overworld", -1, 2))
            .get::<String>("ore")
            .as_deref(),
        Some("copper")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_session_is_never_written_and_updates_count() {
    let session = Store::session("mymod-session");
    session.update("opened", |n: &mut u32| *n += 1);
    assert_eq!(
        Store::session("mymod-session").update("opened", |n: &mut u32| {
            *n += 1;
            *n
        }),
        2
    );
    assert!(session.remove("opened"));
    assert!(!session.contains("opened"));
    session.flush().unwrap();
}
