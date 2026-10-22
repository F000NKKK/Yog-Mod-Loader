//! A mirror end to end through the bus: a provider, keys, a declaration,
//! a tick's writing read in place, a release stopping it.

use std::os::raw::c_void;

use yog_abi::{YogBytes, YogReply, YogStatus, status};
use yog_event::Message;
use yog_event::mirror::{Declare, Key, Keys, Provide, Release, Write};
use yog_ids::key_space;
use yog_mirror::{ColumnRef, FieldSpec, KeySpaceRef, Kind, MirrorSpec, Rate};
use yog_sync::atomic::{AtomicU32, Ordering};

use crate::bus::{Handler, ROUTER, call};

const NAME: &str = "test:mirror/e2e";
const KEY: u64 = 5;
const LEVEL: f32 = 42.0;

static TURNS: AtomicU32 = AtomicU32::new(0);

fn spec() -> MirrorSpec {
    MirrorSpec {
        name: NAME.into(),
        space: key_space::ENTITY.into(),
        fields: vec![FieldSpec {
            name: "level".into(),
            fact: None,
            kind: Kind::F32.code(),
            count: 1,
        }],
    }
}

/// The provider: `LEVEL` for `KEY`'s row, the row marked as filled.
unsafe extern "C" fn write(_: *mut c_void, payload: YogBytes, _: *const YogReply) -> YogStatus {
    TURNS.fetch_add(1, Ordering::Relaxed);
    let Some(w) = Write::from_bytes(unsafe { payload.as_slice() }) else {
        return status::BAD_PAYLOAD;
    };
    let space = unsafe { KeySpaceRef::new(w.space as *const _) }.unwrap();
    let row = space.row_of(KEY).unwrap();
    let rows = unsafe { ColumnRef::new(w.rows as *const _) }.unwrap();
    rows.write().set_bit(row, true);
    let level = unsafe { ColumnRef::new(w.columns[0] as *const _) }.unwrap();
    level.write().set(row, LEVEL);
    status::OK
}

#[test]
fn a_declared_mirror_is_written_and_read_in_place() {
    super::register();
    let at = ROUTER.resolve(NAME);
    assert!(ROUTER.provide(
        at,
        Handler::Native {
            owner: None,
            ud: 0,
            f: write
        }
    ));
    assert_eq!(
        call(&Provide {
            spec: spec(),
            host: false,
            family: false
        }),
        Some(None)
    );
    let rows = call(&Keys {
        space: key_space::ENTITY.into(),
        put: vec![Key {
            key: KEY,
            parent: None,
        }],
        take: Vec::new(),
    })
    .unwrap();
    let declared = call(&Declare {
        spec: spec(),
        wanted: 1,
        rate: Rate::Every(1),
        scope: None,
    })
    .unwrap();
    let d = declared.declaration.unwrap();
    super::after_tick();
    assert_eq!(TURNS.load(Ordering::Relaxed), 1);
    let level = unsafe { ColumnRef::new(d.fields[0].unwrap().column as *const _) }.unwrap();
    let mirror_rows = unsafe { ColumnRef::new(d.rows as *const _) }.unwrap();
    assert_eq!(level.get::<f32>(rows[0]), Some(LEVEL));
    assert!(mirror_rows.bit(rows[0]));
    call(&Release { id: d.id });
    super::after_tick();
    assert_eq!(
        TURNS.load(Ordering::Relaxed),
        1,
        "no one asks: the writer isn't called"
    );
}
