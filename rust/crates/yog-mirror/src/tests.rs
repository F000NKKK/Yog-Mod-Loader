//! The store and its views, as a provider and its readers use them.

use super::*;
use yog_ids::{fact, key_space, mirror_problem};

const THREAT: &str = "test:mirror/threat";
const POS: u32 = 3;

fn field(name: &str, fact: Option<&str>, kind: Kind, count: u32) -> FieldSpec {
    FieldSpec {
        name: name.into(),
        fact: fact.map(str::to_string),
        kind: kind.code(),
        count,
    }
}

/// `pos` linked to the host's fact, `level` its own.
fn threat(space: &str) -> MirrorSpec {
    MirrorSpec {
        name: THREAT.into(),
        space: space.into(),
        fields: vec![
            field("pos", Some(fact::ENTITY_POS), Kind::F64, POS),
            field("level", None, Kind::F32, 1),
        ],
    }
}

/// The host's entity mirror: `pos` only.
fn host_entities() -> MirrorSpec {
    MirrorSpec {
        name: "yog:mirror/entity".into(),
        space: key_space::ENTITY.into(),
        fields: vec![field("pos", Some(fact::ENTITY_POS), Kind::F64, POS)],
    }
}

fn store() -> Store {
    let mut s = Store::default();
    s.space(key_space::ENTITY, None).unwrap();
    s.space(key_space::PLAYER, Some(key_space::ENTITY)).unwrap();
    s
}

/// Who's due at `tick`: (writer, mirror, fields).
fn due(s: &mut Store, tick: u64) -> Vec<(u32, String, u64)> {
    let mut out = DueList::default();
    s.due(tick, &mut out);
    out.turns
        .iter()
        .map(|d| (d.writer, d.mirror.to_string(), d.fields))
        .collect()
}

fn column(d: &Declared, i: usize) -> ColumnRef {
    unsafe { ColumnRef::new(d.fields[i].unwrap().column) }.unwrap()
}

#[test]
fn cells_round_trip() {
    let mut b = [0u8; 24];
    [1.5f64, -2.0, 3.25].put(&mut b);
    assert_eq!(<[f64; 3]>::take(&b), [1.5, -2.0, 3.25]);
    assert_eq!(<[f64; 3]>::WIDTH, 24);
    true.put(&mut b);
    assert!(bool::take(&b));
    assert_eq!(Kind::from_code(Kind::F32.code()), Some(Kind::F32));
}

#[test]
fn keys_get_rows_and_give_them_back() {
    let mut space = KeySpace::new(None);
    let a = space.insert(10, None);
    let b = space.insert(20, None);
    assert_eq!(space.insert(10, None), a, "a key keeps its row");
    assert_ne!(a, b);
    let view = space.view();
    assert_eq!(
        (view.row_of(10), view.row_of(20), view.row_of(30)),
        (Some(a), Some(b), None)
    );
    assert_eq!(space.remove(10), Some(a));
    assert_eq!(view.row_of(10), None);
    assert_eq!(view.key_of(a), None, "a removed row isn't live");
    assert_eq!(view.row_of(20), Some(b), "a probe goes past a gone slot");
    assert_eq!(space.insert(30, None), a, "a row given back is given again");
    assert_eq!(view.live().collect::<Vec<_>>(), vec![(a, 30), (b, 20)]);
}

#[test]
fn keys_grow_past_their_first_buffers() {
    let mut space = KeySpace::new(None);
    let n = 10_000u64;
    for k in 0..n {
        space.insert(k * 7919, None);
    }
    for k in (0..n).step_by(2) {
        space.remove(k * 7919);
    }
    let view = space.view();
    for k in 0..n {
        assert_eq!(view.row_of(k * 7919).is_some(), k % 2 == 1, "key {k}");
    }
    assert_eq!(view.live().count() as u64, n / 2);
}

#[test]
fn a_declaration_reads_what_its_provider_writes() {
    let mut s = store();
    let entity = s.space_id(key_space::ENTITY).unwrap();
    s.provide(&threat(key_space::ENTITY), 1, false).unwrap();
    let d = s
        .declare(&threat(key_space::ENTITY), 0b11, Rate::Every(1), None)
        .unwrap();
    let row = s.insert(entity, 42, None);
    let (pos, level) = (column(&d, 0), column(&d, 1));
    {
        let mut w = pos.write();
        w.set(row, [1.0f64, 2.0, 3.0]);
    }
    level.write().set(row, 0.5f32);
    let space = unsafe { KeySpaceRef::new(d.space) }.unwrap();
    let at = space.row_of(42).unwrap();
    assert_eq!(pos.get::<[f64; 3]>(at), Some([1.0, 2.0, 3.0]));
    assert_eq!(level.get::<f32>(at), Some(0.5));
    let due = due(&mut s, 0);
    assert_eq!(due, vec![(1, THREAT.to_string(), 0b11)]);
}

#[test]
fn values_survive_the_space_growing() {
    let mut s = store();
    let entity = s.space_id(key_space::ENTITY).unwrap();
    let d = s
        .declare(&threat(key_space::ENTITY), 0b10, Rate::Every(1), None)
        .unwrap();
    let level = column(&d, 1);
    let first = s.insert(entity, 0, None);
    level.write().set(first, 7.0f32);
    for k in 1..5_000 {
        s.insert(entity, k, None);
    }
    assert_eq!(level.get::<f32>(first), Some(7.0));
    let last = s.row_of(entity, 4_999).unwrap();
    level.write().set(last, 9.0f32);
    assert_eq!(level.get::<f32>(last), Some(9.0));
}

#[test]
fn a_fact_is_one_column_however_many_link_to_it() {
    let mut s = store();
    let mine = s
        .declare(&threat(key_space::ENTITY), 0b01, Rate::Every(1), None)
        .unwrap();
    let host = s
        .declare(&host_entities(), 0b1, Rate::Every(4), None)
        .unwrap();
    assert_eq!(
        mine.fields[0].unwrap().column,
        host.fields[0].unwrap().column
    );
    assert_eq!(mine.fields[1], None, "level wasn't asked for");
}

#[test]
fn the_host_writes_a_fact_before_any_mod() {
    let mut s = store();
    s.provide(&threat(key_space::ENTITY), 1, false).unwrap();
    let _d = s
        .declare(&threat(key_space::ENTITY), 0b11, Rate::Every(1), None)
        .unwrap();
    assert_eq!(due(&mut s, 0)[0].2, 0b11, "alone, the mod writes pos too");
    s.provide(&host_entities(), 99, true).unwrap();
    let mut now = due(&mut s, 1);
    now.sort_by_key(|d| d.0);
    assert_eq!(
        now,
        vec![
            (1, THREAT.to_string(), 0b10),
            (99, "yog:mirror/entity".to_string(), 0b1)
        ],
        "pos is the host's now"
    );
    s.withdraw(99);
    assert_eq!(due(&mut s, 2)[0].2, 0b11, "the host gone, it's handed back");
}

#[test]
fn demand_merges_and_release_stops_it() {
    let mut s = store();
    s.provide(&threat(key_space::ENTITY), 1, false).unwrap();
    let slow = s
        .declare(&threat(key_space::ENTITY), 0b10, Rate::Every(10), None)
        .unwrap();
    let fast = s
        .declare(&threat(key_space::ENTITY), 0b10, Rate::Every(2), None)
        .unwrap();
    let written = |s: &mut Store, ticks: std::ops::Range<u64>| {
        ticks.filter(|&t| !due(s, t).is_empty()).collect::<Vec<_>>()
    };
    assert_eq!(
        written(&mut s, 0..6),
        vec![0, 2, 4],
        "the most frequent rate wins"
    );
    s.release(fast.id);
    assert_eq!(
        written(&mut s, 6..30),
        vec![6, 16, 26],
        "its turn kept, at the slow rate"
    );
    s.release(slow.id);
    assert!(
        written(&mut s, 30..60).is_empty(),
        "no one asks: nothing is written"
    );
}

#[test]
fn on_change_columns_are_due_when_touched() {
    let mut s = store();
    s.provide(&threat(key_space::ENTITY), 1, false).unwrap();
    let _d = s
        .declare(&threat(key_space::ENTITY), 0b10, Rate::Changes, None)
        .unwrap();
    assert!(due(&mut s, 1).is_empty());
    s.touch(THREAT);
    assert_eq!(due(&mut s, 1).len(), 1);
    assert!(due(&mut s, 2).is_empty(), "a touch is used once");
}

#[test]
fn a_player_mirror_reads_entity_facts_through_its_parent() {
    let mut s = store();
    let entity = s.space_id(key_space::ENTITY).unwrap();
    let player = s.space_id(key_space::PLAYER).unwrap();
    let d = s
        .declare(&threat(key_space::PLAYER), 0b11, Rate::Every(1), None)
        .unwrap();
    let (pos, level) = (d.fields[0].unwrap(), d.fields[1].unwrap());
    assert_eq!((pos.depth, level.depth), (1, 0));
    let entity_row = s.insert(entity, 500, None);
    let player_row = s.insert(player, 7, Some(500));
    let players = unsafe { KeySpaceRef::new(d.space) }.unwrap();
    let entities = players.parent().unwrap();
    assert_eq!(players.parent_key(player_row), Some(500));
    assert_eq!(entities.row_of(500), Some(entity_row));
    let (up, row) = players.up(player_row, pos.depth).unwrap();
    assert_eq!((up.as_ptr(), row), (entities.as_ptr(), entity_row));
    let at = unsafe { ColumnRef::new(pos.column) }.unwrap();
    at.write().set(entity_row, [1.0f64, 2.0, 3.0]);
    assert_eq!(at.get::<[f64; 3]>(row), Some([1.0, 2.0, 3.0]), "a player's pos is its entity's");
}

#[test]
fn problems_are_coded() {
    let mut s = store();
    let mut bad = threat(key_space::ENTITY);
    bad.fields[0].count = 2;
    s.declare(&threat(key_space::ENTITY), 0b01, Rate::Every(1), None)
        .unwrap();
    let mut other = host_entities();
    other.fields[0].count = 2;
    let code = |e: yog_data::Diagnostic| e.code.map(|c| c.to_string());
    assert_eq!(
        code(s.declare(&other, 1, Rate::Every(1), None).unwrap_err()).as_deref(),
        Some(mirror_problem::WRONG_SHAPE)
    );
    assert_eq!(
        code(s.declare(&bad, 1, Rate::Every(1), None).unwrap_err()).as_deref(),
        Some(mirror_problem::TAKEN)
    );
    let mut foreign = host_entities();
    foreign.name = "test:mirror/foreign".into();
    foreign.fields[0].fact = Some(fact::PLAYER_NAME.into());
    assert_eq!(
        code(s.declare(&foreign, 1, Rate::Every(1), None).unwrap_err()).as_deref(),
        Some(mirror_problem::FOREIGN_FACT)
    );
    s.provide(&threat(key_space::ENTITY), 1, false).unwrap();
    assert_eq!(
        code(s.provide(&threat(key_space::ENTITY), 2, false).unwrap_err()).as_deref(),
        Some(mirror_problem::TAKEN)
    );
}

#[test]
fn the_host_keeps_keys_while_anyone_reads_its_space() {
    let mut s = store();
    s.provide(&host::ENTITY.spec(), 99, true).unwrap();
    assert!(due(&mut s, 0).is_empty(), "no one reads entities");
    let players = MirrorSpec {
        name: "test:mirror/seen".into(),
        space: key_space::PLAYER.into(),
        fields: vec![field("pos", Some(fact::ENTITY_POS), Kind::F64, POS)],
    };
    let d = s.declare(&players, 0, Rate::Every(1), None).unwrap();
    assert_eq!(
        due(&mut s, 1),
        vec![(99, host::ENTITY.address.to_string(), 0)],
        "a part of it is read"
    );
    s.release(d.id);
    assert!(due(&mut s, 2).is_empty());
}

#[test]
fn a_family_member_is_made_when_named_with_its_templates() {
    let mut s = store();
    s.family(key_space::BLOCK);
    s.provide_family(&host::BLOCK.spec(), 7, true).unwrap();
    let overworld = format!("{}/minecraft:overworld", key_space::BLOCK);
    let at = |name: &str| MirrorSpec {
        name: format!("{name}/minecraft:overworld"),
        space: overworld.clone(),
        fields: vec![field("state", Some(fact::BLOCK_STATE), Kind::U32, 1)],
    };
    let scope = vec![1u8, 2, 3];
    let host_view = s
        .declare(
            &at(host::BLOCK.address),
            1,
            Rate::Every(1),
            Some(scope.clone()),
        )
        .unwrap();
    let mine = s
        .declare(&at("test:mirror/flow"), 1, Rate::Every(1), None)
        .unwrap();
    assert_eq!(
        host_view.fields[0].unwrap().column,
        mine.fields[0].unwrap().column,
        "a family's fact is one column in its member"
    );
    let mut out = DueList::default();
    s.due(0, &mut out);
    let [turn] = &out.turns[..] else {
        panic!("one turn: {:?}", out.turns);
    };
    assert_eq!(
        (turn.writer, &*turn.mirror),
        (7, at(host::BLOCK.address).name.as_str())
    );
    assert_eq!(
        out.scopes(turn)
            .iter()
            .map(|s| s.to_vec())
            .collect::<Vec<_>>(),
        vec![scope]
    );
    let nether = MirrorSpec {
        name: "test:mirror/far".into(),
        space: format!("{}/minecraft:the_nether", key_space::BLOCK),
        fields: Vec::new(),
    };
    s.declare(&nether, 0, Rate::Every(1), None).unwrap();
    s.due(1, &mut out);
    assert_eq!(out.turns.len(), 2, "the new member got its host mirror too");
}
