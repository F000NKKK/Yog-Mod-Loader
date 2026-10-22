//! Mirrors' hot paths (`docs/design/bus.md`, "Performance": a hot read
//! 1–5 ns, a change 10–20 ns): a value read by row, by key, every live
//! row walked, a tick's writing, the store working out what's due, keys
//! coming and going.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use yog_ids::{fact, key_space};
use yog_mirror::{ColumnRef, Declared, FieldSpec, KeySpaceRef, Kind, MirrorSpec, Rate, Store};

/// Entities in a busy world.
const ENTITIES: u64 = 1_000;
/// Network ids are handed out in order, with gaps where entities left.
const ID_STEP: u64 = 3;
const POS: u32 = 3;
/// Mirrors a modded game might have.
const MIRRORS: usize = 32;

fn spec(name: &str) -> MirrorSpec {
    MirrorSpec {
        name: name.into(),
        space: key_space::ENTITY.into(),
        fields: vec![
            FieldSpec {
                name: "pos".into(),
                fact: Some(fact::ENTITY_POS.into()),
                kind: Kind::F64.code(),
                count: POS,
            },
            FieldSpec {
                name: "level".into(),
                fact: None,
                kind: Kind::F32.code(),
                count: 1,
            },
        ],
    }
}

/// A store with `ENTITIES` entities, a mirror of their positions and a level.
fn world() -> (Store, Declared) {
    let mut s = Store::default();
    let entity = s.space(key_space::ENTITY, None).unwrap();
    s.provide(&spec("bench:mirror/threat"), 1, false).unwrap();
    let d = s
        .declare(&spec("bench:mirror/threat"), 0b11, Rate::Every(1))
        .unwrap();
    for k in 0..ENTITIES {
        s.insert(entity, k * ID_STEP, None);
    }
    let pos = unsafe { ColumnRef::new(d.fields[0].unwrap().column) }.unwrap();
    let mut w = pos.write();
    for row in 0..ENTITIES as u32 {
        w.set(row, [row as f64, 64.0, -(row as f64)]);
    }
    drop(w);
    (s, d)
}

fn reads(c: &mut Criterion) {
    let (_s, d) = world();
    let pos = unsafe { ColumnRef::new(d.fields[0].unwrap().column) }.unwrap();
    let level = unsafe { ColumnRef::new(d.fields[1].unwrap().column) }.unwrap();
    let space = unsafe { KeySpaceRef::new(d.space) }.unwrap();
    let mut g = c.benchmark_group("read");
    g.bench_function("f32 by row", |b| {
        b.iter(|| level.get::<f32>(black_box(500)))
    });
    g.bench_function("pos by row", |b| {
        b.iter(|| pos.get::<[f64; 3]>(black_box(500)))
    });
    g.bench_function("row of key", |b| {
        b.iter(|| space.row_of(black_box(500 * ID_STEP)))
    });
    g.bench_function("pos by key", |b| {
        b.iter(|| {
            space
                .row_of(black_box(500 * ID_STEP))
                .and_then(|r| pos.get::<[f64; 3]>(r))
        })
    });
    g.throughput(Throughput::Elements(ENTITIES));
    g.bench_function("every live pos", |b| {
        b.iter(|| {
            space
                .live_rows()
                .filter_map(|row| pos.get::<[f64; 3]>(row))
                .fold(0.0, |sum, p| sum + p[1])
        })
    });
    g.finish();
}

fn writes(c: &mut Criterion) {
    let (_s, d) = world();
    let pos = unsafe { ColumnRef::new(d.fields[0].unwrap().column) }.unwrap();
    let mut g = c.benchmark_group("write");
    g.throughput(Throughput::Elements(ENTITIES));
    g.bench_function("a tick of pos", |b| {
        b.iter(|| {
            let mut w = pos.write();
            for row in 0..ENTITIES as u32 {
                w.set(row, black_box([row as f64, 65.0, 1.0]));
            }
        })
    });
    g.finish();
}

fn store(c: &mut Criterion) {
    let mut g = c.benchmark_group("store");
    for mirrors in [1, MIRRORS] {
        let mut s = Store::default();
        s.space(key_space::ENTITY, None).unwrap();
        for m in 0..mirrors {
            let name = format!("bench:mirror/m{m}");
            s.provide(&spec(&name), m as u32, false).unwrap();
            s.declare(&spec(&name), 0b11, Rate::Every(1)).unwrap();
        }
        g.bench_with_input(BenchmarkId::new("due", mirrors), &mirrors, |b, _| {
            let (mut tick, mut out) = (0, yog_mirror::DueList::default());
            b.iter(|| {
                tick += 1;
                s.due(tick, &mut out);
                out.turns.len()
            })
        });
    }
    let (mut s, _d) = world();
    let entity = s.space_id(key_space::ENTITY).unwrap();
    let mut next = ENTITIES * ID_STEP;
    g.bench_function("key in and out", |b| {
        b.iter(|| {
            next += 1;
            s.insert(entity, next, None);
            s.remove(entity, next)
        })
    });
    g.finish();
}

criterion_group!(benches, reads, writes, store);
criterion_main!(benches);
