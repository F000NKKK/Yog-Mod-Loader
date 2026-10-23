//! `.yui` parsing and code generation: plain screens (`yui`), view-model
//! screens (`yui_vm`), components (`yui_component`) and `<Grid>` lowering
//! (`yui_grid`), on synthetic markup of growing size.

mod common;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use yog_cli::ui::{grid, yui};

/// Synthetic markup of `n` elements.
type Markup = fn(usize) -> String;

fn bench(c: &mut Criterion) {
    let kinds: [(&str, Markup); 3] = [
        ("yui", common::screen),
        ("yui_vm", common::vm_screen),
        ("yui_component", common::component),
    ];
    for (kind, make) in kinds {
        let mut parse = c.benchmark_group(format!("parse/{kind}"));
        for n in common::SIZES {
            let src = make(n);
            parse.bench_with_input(BenchmarkId::from_parameter(n), &src, |b, src| {
                b.iter(|| yui::parse_xml(black_box(src)).unwrap())
            });
        }
        parse.finish();
        let mut generate = c.benchmark_group(format!("generate/{kind}"));
        for n in common::SIZES {
            let src = make(n);
            generate.bench_with_input(BenchmarkId::from_parameter(n), &src, |b, src| {
                b.iter(|| {
                    yui::generate_for(black_box(src), "example", Some(common::MOD_ID)).unwrap()
                })
            });
        }
        generate.finish();
    }

    let mut grid = c.benchmark_group("yui_grid/normalize");
    for n in common::SIZES {
        let parsed = yui::parse_xml(&common::vm_screen(n)).unwrap();
        grid.bench_with_input(BenchmarkId::from_parameter(n), &parsed, |b, parsed| {
            b.iter(|| {
                let mut root = parsed.clone();
                grid::normalize(black_box(&mut root)).unwrap();
                root
            })
        });
    }
    grid.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
