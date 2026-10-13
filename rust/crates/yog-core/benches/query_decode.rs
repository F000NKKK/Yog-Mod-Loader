//! Decoding the host's tab-separated world-query answers.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use yog_core::{BlockHit, BlockInfo, EntityInfo, PlayerInfo};

fn bench(c: &mut Criterion) {
    let entity = "minecraft:zombie\tminecraft:overworld\t1.5\t64\t-2\t1.74\t0.6\t1.95\t169\tZombie";
    let hit = "1\t2\t3\t5\t1.0\t2.5\t3.5";
    let block = "minecraft:water[level=0]\t100\t1\t0\tminecraft:water\t1\t3\t1";
    let player = "0f8e1c2a-0000-4000-8000-000000000000\tcreative\t4\t30\t1395\t300\t1\t0";

    let mut g = c.benchmark_group("query_decode");
    g.bench_function("entity", |b| {
        b.iter(|| EntityInfo::decode(black_box(entity)))
    });
    g.bench_function("block_hit", |b| b.iter(|| BlockHit::decode(black_box(hit))));
    g.bench_function("block", |b| b.iter(|| BlockInfo::decode(black_box(block))));
    g.bench_function("player", |b| {
        b.iter(|| PlayerInfo::decode(black_box(player)))
    });
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
