//! Packet encode/decode for a typical small packet and a blob-carrying one.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use yog_network::{Packet, packet};

packet! {
    struct Teleport {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        on_ground: bool,
        player: String,
    }
}

packet! {
    struct Chunk {
        cx: i32,
        cz: i32,
        seq: u64,
        name: String,
        data: Vec<u8>,
    }
}

fn bench(c: &mut Criterion) {
    let tp = Teleport {
        x: 12.5,
        y: 64.0,
        z: -301.25,
        yaw: 90.0,
        on_ground: true,
        player: "Player_0123456789".into(),
    };
    let chunk = Chunk {
        cx: 3,
        cz: -7,
        seq: 42,
        name: "example:section".into(),
        data: (0..4096u32).map(|i| i as u8).collect(),
    };
    let tp_bytes = tp.encode();
    let chunk_bytes = chunk.encode();

    let mut g = c.benchmark_group("packet");
    g.bench_function("encode_small", |b| b.iter(|| black_box(&tp).encode()));
    g.bench_function("decode_small", |b| {
        b.iter(|| Teleport::decode(black_box(&tp_bytes)).unwrap());
    });
    g.bench_function("encode_4k", |b| b.iter(|| black_box(&chunk).encode()));
    g.bench_function("decode_4k", |b| {
        b.iter(|| Chunk::decode(black_box(&chunk_bytes)).unwrap());
    });
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
