//! Yog Data against serde_json on a real document (HexMod's book, 145 KB):
//! parsing the text, writing it, and the binary form.

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

const BOOK: &str = include_str!("hexbook.ybook");

fn bench(c: &mut Criterion) {
    let value = yog_data::text::parse_value(BOOK).unwrap();
    let bytes = yog_data::binary::encode(&value);
    let json: serde_json::Value = serde_json::from_str(BOOK).unwrap();
    eprintln!("text {} bytes, binary {} bytes", BOOK.len(), bytes.len());

    let mut g = c.benchmark_group("parse");
    g.bench_function("serde_json", |b| {
        b.iter(|| serde_json::from_str::<serde_json::Value>(black_box(BOOK)).unwrap())
    });
    g.bench_function("yog_data text", |b| {
        b.iter(|| yog_data::text::parse_value(black_box(BOOK)).unwrap())
    });
    g.bench_function("yog_data text + spans", |b| {
        b.iter(|| yog_data::text::parse(black_box(BOOK)).unwrap())
    });
    g.bench_function("yog_data binary", |b| {
        b.iter(|| yog_data::binary::decode(black_box(&bytes)).unwrap())
    });
    g.finish();

    let mut g = c.benchmark_group("write");
    g.bench_function("serde_json", |b| {
        b.iter(|| serde_json::to_string(black_box(&json)).unwrap())
    });
    g.bench_function("yog_data text", |b| {
        b.iter(|| yog_data::text::to_string(black_box(&value)))
    });
    g.bench_function("yog_data binary", |b| {
        b.iter(|| yog_data::binary::encode(black_box(&value)))
    });
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
