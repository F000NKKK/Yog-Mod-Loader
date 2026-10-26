//! A store's records — Yog Script objects' state — as a world save writes
//! and a chunk load reads them: one chunk's worth of block records set,
//! read back, and removed by a block's prefix; against the typed path
//! (`set`/`get`) of the same data.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use yog_data::Schema;
use yog_storage::Store;

/// A block's state: a charger's.
#[derive(Schema, Clone, Default)]
struct Charger {
    energy: i64,
    owner: String,
    mode: u8,
}

/// How many blocks of a chunk have state: few, a machine room, a full one.
const BLOCKS: [usize; 3] = [16, 256, 4096];

fn key(i: usize) -> String {
    format!("{}_{}_{}/charge.Charger", i % 16, 64 + i / 256, (i / 16) % 16)
}

fn state(i: usize) -> Charger {
    Charger {
        energy: i as i64 * 1000,
        owner: "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(),
        mode: (i % 3) as u8,
    }
}

fn records(c: &mut Criterion) {
    let mut group = c.benchmark_group("records");
    for n in BLOCKS {
        let bytes: Vec<(String, Vec<u8>)> = (0..n)
            .map(|i| (key(i), yog_data::to_bytes(&state(i))))
            .collect();
        group.bench_with_input(BenchmarkId::new("set_record", n), &bytes, |b, bytes| {
            let store = Store::session("bench-set");
            b.iter(|| {
                for (k, v) in bytes {
                    store.set_record(k, v);
                }
            });
        });
        group.bench_with_input(BenchmarkId::new("set_typed", n), &n, |b, &n| {
            let store = Store::session("bench-typed");
            let keys: Vec<String> = (0..n).map(key).collect();
            b.iter(|| {
                for (i, k) in keys.iter().enumerate() {
                    store.set(k, &state(i));
                }
            });
        });
        let store = Store::session("bench-read");
        for (k, v) in &bytes {
            store.set_record(k, v);
        }
        group.bench_with_input(BenchmarkId::new("record", n), &bytes, |b, bytes| {
            b.iter(|| {
                for (k, _) in bytes {
                    black_box(store.record(k));
                }
            });
        });
        group.bench_with_input(BenchmarkId::new("remove_under", n), &bytes, |b, bytes| {
            b.iter(|| {
                let store = Store::session("bench-remove");
                for (k, v) in bytes {
                    store.set_record(k, v);
                }
                black_box(store.remove_under("0_64_0/"))
            });
        });
    }
    group.finish();
}

criterion_group!(benches, records);
criterion_main!(benches);
