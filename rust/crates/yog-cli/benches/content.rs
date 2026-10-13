//! `content/` blocks and items: parsing a file, compiling a mod's content
//! (models, states, loot, tags, lang, Rust glue) and merging its outputs.

mod common;

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use yog_cli::{content, yres};

fn bench(c: &mut Criterion) {
    let root = common::mod_root("hexmod");
    let files = content::find(&root);
    let biggest = files
        .iter()
        .max_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .unwrap();
    let text = common::read(biggest);
    c.bench_function("content/json_parse/biggest", |b| {
        b.iter(|| content::json::parse(black_box(&text)).unwrap())
    });

    let res = yres::compile(&root, "hexcasting").unwrap();
    c.bench_function("content/compile/hexmod", |b| {
        b.iter(|| content::compile(black_box(&root), "hexcasting", &res.refs).unwrap())
    });
    c.bench_function("content/compile_merge/hexmod", |b| {
        b.iter(|| {
            let compiled = content::compile(&root, "hexcasting", &res.refs).unwrap();
            let mut outputs = res.outputs.clone();
            content::merge_outputs(&root, &mut outputs, compiled).unwrap();
            outputs
        })
    });
    let _ = std::fs::remove_dir_all(&root);
}

criterion_group!(benches, bench);
criterion_main!(benches);
