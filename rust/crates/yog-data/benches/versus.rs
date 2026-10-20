//! Yog Data against the formats it replaced, on the same structs: rkyv 0.8
//! (mods' exports), postcard (serde's compact binary) and serde_json (the
//! client's operations). Each is written and read back whole; rkyv is also
//! read in place (validated access, no copy). Yog Data both ways: the
//! tagged binary form (files, self-describing) and frames (the bus:
//! positional, written into a reused buffer, read whole or as a view).

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

macro_rules! all_formats {
    ($($item:item)*) => {$(
        #[derive(yog_data::Schema, serde::Serialize, serde::Deserialize, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, PartialEq, Debug)]
        #[data(view)]
        $item
    )*};
}

all_formats! {
    struct Point { x: f64, y: f64, z: f64 }
    struct EntityInfo { type_id: String, dimension: String, pos: Point, eye_height: f64, width: f64, height: f64, flags: u32, name: String }
    struct Stack { item: String, count: u32, snbt: String }
    struct SlotStack { slot: u32, stack: Stack }
    struct Inventory { stacks: Vec<SlotStack> }
    struct Area { palette: Vec<String>, blocks: Vec<u16> }
    struct Placed { id: String, count: u32 }
    struct Placements { items: Vec<Placed> }
    struct Floats { items: Vec<f64> }
}

const SLOTS: u32 = 41;
const AREA: usize = 32 * 32 * 32;
const PLACED: usize = 10_000;
const KINDS: usize = 16;
const FLOATS: usize = 10_000;

fn entity() -> EntityInfo {
    EntityInfo {
        type_id: "minecraft:zombie".into(),
        dimension: "minecraft:overworld".into(),
        pos: Point {
            x: 12.5,
            y: 64.0,
            z: -3.25,
        },
        eye_height: 1.74,
        width: 0.6,
        height: 1.95,
        flags: 169,
        name: "Zombie".into(),
    }
}

fn inventory() -> Inventory {
    Inventory {
        stacks: (0..SLOTS)
            .map(|slot| SlotStack {
                slot,
                stack: Stack {
                    item: format!("minecraft:item_{}", slot % 7),
                    count: slot % 64 + 1,
                    snbt: "{}".into(),
                },
            })
            .collect(),
    }
}

fn area() -> Area {
    Area {
        palette: vec![
            "minecraft:stone".into(),
            "minecraft:air".into(),
            "minecraft:dirt".into(),
        ],
        blocks: (0..AREA).map(|i| (i % 3) as u16).collect(),
    }
}

fn placements() -> Placements {
    Placements {
        items: (0..PLACED)
            .map(|i| Placed {
                id: format!("minecraft:block_{}", i % KINDS),
                count: i as u32,
            })
            .collect(),
    }
}

fn floats() -> Floats {
    Floats {
        items: (0..FLOATS).map(|i| i as f64 * 0.5).collect(),
    }
}

macro_rules! compare {
    ($c:expr, $name:literal, $value:expr, $ty:ty, |$view:ident| $ours:expr, |$arch:ident| $theirs:expr) => {{
        let v: $ty = $value;
        let yog = yog_data::to_bytes(&v);
        let frame = yog_data::frame::to_frame(&v);
        let post = postcard::to_allocvec(&v).unwrap();
        let json = serde_json::to_vec(&v).unwrap();
        let rk = rkyv::to_bytes::<rkyv::rancor::Error>(&v).unwrap();
        eprintln!(
            "{}: yog {} B, frame {} B, postcard {} B, json {} B, rkyv {} B",
            $name,
            yog.len(),
            frame.len(),
            post.len(),
            json.len(),
            rk.len()
        );
        let mut g = $c.benchmark_group($name);
        g.bench_function("yog encode", |b| {
            b.iter(|| yog_data::to_bytes(black_box(&v)))
        });
        g.bench_function("frame encode", |b| {
            b.iter(|| yog_data::frame::to_frame(black_box(&v)))
        });
        g.bench_function("frame encode reused", |b| {
            b.iter(|| yog_data::frame::with_frame(black_box(&v), |f| f.len()))
        });
        g.bench_function("postcard encode", |b| {
            b.iter(|| postcard::to_allocvec(black_box(&v)).unwrap())
        });
        g.bench_function("json encode", |b| {
            b.iter(|| serde_json::to_vec(black_box(&v)).unwrap())
        });
        g.bench_function("rkyv encode", |b| {
            b.iter(|| rkyv::to_bytes::<rkyv::rancor::Error>(black_box(&v)).unwrap())
        });
        g.bench_function("yog decode", |b| {
            b.iter(|| yog_data::read_bytes::<$ty>(black_box(&yog)).unwrap())
        });
        g.bench_function("frame decode", |b| {
            b.iter(|| yog_data::frame::read_frame::<$ty>(black_box(&frame)).unwrap())
        });
        g.bench_function("frame view", |b| {
            b.iter(|| {
                yog_data::frame::read_view::<$ty>(black_box(&frame))
                    .map(|_| ())
                    .unwrap()
            })
        });
        g.bench_function("postcard decode", |b| {
            b.iter(|| postcard::from_bytes::<$ty>(black_box(&post)).unwrap())
        });
        g.bench_function("json decode", |b| {
            b.iter(|| serde_json::from_slice::<$ty>(black_box(&json)).unwrap())
        });
        g.bench_function("rkyv decode", |b| {
            b.iter(|| rkyv::from_bytes::<$ty, rkyv::rancor::Error>(black_box(&rk)).unwrap())
        });
        g.bench_function("rkyv access in place", |b| {
            b.iter(|| {
                rkyv::access::<rkyv::Archived<$ty>, rkyv::rancor::Error>(black_box(&rk))
                    .map(|_| ())
                    .unwrap()
            })
        });
        // One value read out of the bytes, opening included: what a call does.
        g.bench_function("frame view + one value", |b| {
            b.iter(|| {
                let $view = yog_data::frame::read_view::<$ty>(black_box(&frame)).unwrap();
                black_box($ours)
            })
        });
        g.bench_function("rkyv access + one value", |b| {
            b.iter(|| {
                let $arch =
                    rkyv::access::<rkyv::Archived<$ty>, rkyv::rancor::Error>(black_box(&rk))
                        .unwrap();
                black_box($theirs)
            })
        });
        g.finish();
    }};
}

fn bench(c: &mut Criterion) {
    compare!(c, "entity_info", entity(), EntityInfo, |v| v.pos.x, |a| a
        .pos
        .x
        .to_native());
    compare!(
        c,
        "inventory",
        inventory(),
        Inventory,
        |v| v.stacks.get(40).map(|s| s.stack.count),
        |a| a.stacks[40].stack.count.to_native()
    );
    compare!(c, "area_32", area(), Area, |v| v.blocks.get(30_000), |a| a
        .blocks[30_000]
        .to_native());
    compare!(
        c,
        "placed_10k",
        placements(),
        Placements,
        |v| v.items.get(9_999).map(|p| p.count),
        |a| a.items[9_999].count.to_native()
    );
    compare!(
        c,
        "floats_10k",
        floats(),
        Floats,
        |v| v.items.get(9_999),
        |a| a.items[9_999].to_native()
    );
}

criterion_group!(benches, bench);
criterion_main!(benches);
