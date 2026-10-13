//! `.yres` resources: parsing a set, compiling a mod's resources.

mod common;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box;
use yog_cli::yres;

fn bench(c: &mut Criterion) {
    let mut parse = c.benchmark_group("yres/parse");
    for n in common::SIZES {
        let (neutral, _) = common::yres_set(n);
        parse.bench_with_input(BenchmarkId::from_parameter(n), &neutral, |b, text| {
            b.iter(|| yres::parse(black_box(text)).unwrap())
        });
    }
    parse.finish();

    let mut compile = c.benchmark_group("yres/compile");
    for n in common::SIZES {
        let root = common::mod_root(n);
        compile.bench_with_input(BenchmarkId::from_parameter(n), &root, |b, root| {
            b.iter(|| yres::compile(black_box(root), common::MOD_ID).unwrap())
        });
        common::remove(&root);
    }
    compile.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
