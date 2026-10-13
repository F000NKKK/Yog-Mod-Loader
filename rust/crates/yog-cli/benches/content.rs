//! `content/` blocks and items: parsing a file, compiling a mod's content
//! (models, states, loot, tags, lang, Rust glue) and merging its outputs.
//!
//! `YOG_BENCH_MOD_DIR=<mod root>` also benches a real mod's content.

mod common;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::path::Path;
use yog_cli::{content, manifest, yres};

fn bench(c: &mut Criterion) {
    let block = common::yblock(1);
    c.bench_function("content/json_parse/yblock", |b| {
        b.iter(|| content::json::parse(black_box(&block)).unwrap())
    });

    let mut compile = c.benchmark_group("content/compile");
    for n in common::SIZES {
        let root = common::mod_root(n);
        let res = yres::compile(&root, common::MOD_ID).unwrap();
        compile.bench_with_input(BenchmarkId::from_parameter(n), &root, |b, root| {
            b.iter(|| content::compile(black_box(root), common::MOD_ID, &res.refs).unwrap())
        });
        common::remove(&root);
    }
    compile.finish();

    let mut merge = c.benchmark_group("content/compile_merge");
    for n in common::SIZES {
        let root = common::mod_root(n);
        let res = yres::compile(&root, common::MOD_ID).unwrap();
        merge.bench_with_input(BenchmarkId::from_parameter(n), &root, |b, root| {
            b.iter(|| {
                let compiled = content::compile(root, common::MOD_ID, &res.refs).unwrap();
                let mut outputs = res.outputs.clone();
                content::merge_outputs(root, &mut outputs, compiled).unwrap();
                outputs
            })
        });
        common::remove(&root);
    }
    merge.finish();

    if let Ok(dir) = std::env::var("YOG_BENCH_MOD_DIR") {
        let root = Path::new(&dir);
        let id = manifest::YogToml::read(&root.join("yog.toml")).unwrap().id;
        let res = yres::compile(root, &id).unwrap();
        c.bench_function("content/compile/YOG_BENCH_MOD_DIR", |b| {
            b.iter(|| content::compile(black_box(root), &id, &res.refs).unwrap())
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
