//! `.yres` resources: parsing one file, compiling a mod's whole set.

mod common;

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use yog_cli::yres;

fn bench(c: &mut Criterion) {
    let lang = common::read(&common::fixture("hexmod/resources/Lang.yres"));
    c.bench_function("yres/parse/Lang", |b| {
        b.iter(|| yres::parse(black_box(&lang)).unwrap())
    });

    for (name, id) in [("hexmod", "hexcasting"), ("modlist", "yog-modlist")] {
        let root = common::mod_root(name);
        c.bench_function(&format!("yres/compile/{name}"), |b| {
            b.iter(|| yres::compile(black_box(&root), id).unwrap())
        });
        let _ = std::fs::remove_dir_all(&root);
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
