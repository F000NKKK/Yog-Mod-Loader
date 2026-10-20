//! The bus's path for typed messages: a struct written to Yog Data's binary
//! form (`write` + `binary::encode`) and read back (`read_bytes`), for a
//! small, a medium and a large message — against a hand-written byte
//! layout of the same data, the ceiling for any encoding.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use yog_data::Schema;

#[derive(Schema, Clone, PartialEq, Debug)]
struct Point {
    x: f64,
    y: f64,
    z: f64,
}

/// Small: an entity's state (`yog_core::EntityInfo`).
#[derive(Schema, Clone, PartialEq, Debug)]
struct EntityInfo {
    type_id: String,
    dimension: String,
    pos: Point,
    eye_height: f64,
    width: f64,
    height: f64,
    flags: u32,
    name: String,
}

#[derive(Schema, Clone, PartialEq, Debug)]
struct Stack {
    item: String,
    count: u32,
    snbt: String,
}

/// Medium: an inventory (`Vec<yog_core::SlotStack>`).
#[derive(Schema, Clone, PartialEq, Debug)]
struct SlotStack {
    slot: u32,
    stack: Stack,
}

#[derive(Schema, Clone, PartialEq, Debug)]
struct Inventory {
    stacks: Vec<SlotStack>,
}

/// Large: an area of blocks (`yog_core::BlockArea`), 32³.
#[derive(Schema, Clone, PartialEq, Debug)]
struct Area {
    palette: Vec<String>,
    blocks: Vec<u16>,
}

const AREA_SIDE: usize = 32;
const INVENTORY_SLOTS: u32 = 41;

fn entity() -> EntityInfo {
    EntityInfo {
        type_id: "minecraft:zombie".into(),
        dimension: "minecraft:overworld".into(),
        pos: Point { x: 12.5, y: 64.0, z: -3.25 },
        eye_height: 1.74,
        width: 0.6,
        height: 1.95,
        flags: 169,
        name: "Zombie".into(),
    }
}

fn inventory() -> Inventory {
    Inventory {
        stacks: (0..INVENTORY_SLOTS)
            .map(|slot| SlotStack { slot, stack: Stack { item: format!("minecraft:item_{}", slot % 7), count: slot % 64 + 1, snbt: "{}".into() } })
            .collect(),
    }
}

fn area() -> Area {
    let n = AREA_SIDE * AREA_SIDE * AREA_SIDE;
    Area { palette: vec!["minecraft:stone".into(), "minecraft:air".into(), "minecraft:dirt".into()], blocks: (0..n).map(|i| (i % 3) as u16).collect() }
}

fn encode<T: Schema>(v: &T) -> Vec<u8> {
    yog_data::binary::encode(&v.write())
}

/// The hand-written layout: little-endian numbers, strings a u16 length.
mod raw {
    pub fn str(o: &mut Vec<u8>, s: &str) {
        o.extend_from_slice(&(s.len() as u16).to_le_bytes());
        o.extend_from_slice(s.as_bytes());
    }

    pub fn entity(e: &super::EntityInfo) -> Vec<u8> {
        let mut o = Vec::with_capacity(96);
        str(&mut o, &e.type_id);
        str(&mut o, &e.dimension);
        for f in [e.pos.x, e.pos.y, e.pos.z, e.eye_height, e.width, e.height] {
            o.extend_from_slice(&f.to_le_bytes());
        }
        o.extend_from_slice(&e.flags.to_le_bytes());
        str(&mut o, &e.name);
        o
    }

    pub fn area(a: &super::Area) -> Vec<u8> {
        let mut o = Vec::with_capacity(a.blocks.len() * 2 + 64);
        for p in &a.palette {
            str(&mut o, p);
        }
        for b in &a.blocks {
            o.extend_from_slice(&b.to_le_bytes());
        }
        o
    }
}

fn case<T: Schema + PartialEq + std::fmt::Debug>(c: &mut Criterion, name: &str, v: &T, raw: Option<fn(&T) -> Vec<u8>>) {
    let bytes = encode(v);
    assert_eq!(&yog_data::read_bytes::<T>(&bytes).unwrap(), v);
    let mut g = c.benchmark_group(name);
    g.throughput(Throughput::Bytes(bytes.len() as u64));
    g.bench_function(BenchmarkId::new("encode", bytes.len()), |b| b.iter(|| encode(black_box(v))));
    g.bench_function(BenchmarkId::new("decode", bytes.len()), |b| b.iter(|| yog_data::read_bytes::<T>(black_box(&bytes)).unwrap()));
    if let Some(raw) = raw {
        g.bench_function(BenchmarkId::new("raw encode", raw(v).len()), |b| b.iter(|| raw(black_box(v))));
    }
    g.finish();
}

fn bench(c: &mut Criterion) {
    case(c, "entity_info", &entity(), Some(raw::entity));
    case(c, "inventory", &inventory(), None);
    case(c, "area_32", &area(), Some(raw::area));
}

criterion_group!(benches, bench);
criterion_main!(benches);
