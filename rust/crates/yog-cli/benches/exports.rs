//! `yog publish exports` codegen: scanning sources for `#[yog_export]`
//! items, resolving their imports, generating the crate's `lib.rs`.

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use yog_cli::project::exports;

/// A mod source with `n` exported structs, enums and fns, `use` lines and
/// unexported filler between them.
fn source(n: usize) -> String {
    let mut s = String::from(
        "use std::collections::HashMap;\nuse yog_api::{Registry, BlockPos};\nuse yog_api::registry::Mod;\nuse crate::kind::Kind;\n\n",
    );
    for i in 0..n {
        s.push_str(&format!(
            "/// Filler {i}.\nfn helper_{i}(x: u32) -> u32 {{\n    x * {i}\n}}\n\n\
             #[yog_export]\n#[derive(Debug, Clone)]\npub struct Def{i} {{\n    pub name: String,\n    pub pos: BlockPos,\n    pub map: HashMap<String, Vec<u32>>,\n    pub kind: Kind,\n}}\n\n\
             #[yog_export]\npub enum Mode{i} {{\n    A,\n    B(u32),\n    C {{ x: f64 }},\n}}\n\n\
             #[yog_export]\npub fn register_{i}(\n    registry: &mut Registry,\n    def: Def{i},\n    mode: Mode{i},\n    extra: HashMap<String, (u8, u16)>,\n) -> Result<Option<BlockPos>, String> {{\n    let _ = (registry, def, mode, extra);\n    Ok(None)\n}}\n\n"
        ));
    }
    s
}

fn bench(c: &mut Criterion) {
    let src = source(20);
    let sources = vec![src.clone()];
    c.bench_function("exports/scan/20", |b| {
        b.iter(|| exports::scan(black_box(&src)))
    });
    let items = exports::scan(&src);
    c.bench_function("exports/collect_uses/20", |b| {
        b.iter(|| exports::collect_uses(black_box(&items), &sources))
    });
    let (system, types) = exports::collect_uses(&items, &sources);
    c.bench_function("exports/lib_rs/20", |b| {
        b.iter(|| exports::lib_rs("m", "1.0.0", black_box(&items), &system, &types))
    });
    let fn_src = &items
        .iter()
        .find(|i| i.kind == exports::ExportKind::Fn)
        .unwrap()
        .source;
    c.bench_function("exports/import_signature", |b| {
        b.iter(|| exports::import_signature(black_box(fn_src)))
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
