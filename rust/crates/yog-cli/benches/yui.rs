//! `.yui` parsing and code generation: plain screens (`yui`), view-model
//! screens (`yui_vm`), components (`yui_component`) and `<Grid>` lowering
//! (`yui_grid`).

mod common;

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use yog_cli::{yui, yui_grid};

/// A plain screen with `n` rows of widgets — scales the parser.
fn big_screen(n: usize) -> String {
    let mut s = String::from("<Screen id=\"m:big\" width=\"400\" height=\"300\">\n  <Column w=\"100%\" gap=\"2\">\n");
    for i in 0..n {
        s.push_str(&format!(
            "    <Row gap=\"4\" align=\"center\">\n      <Label text=\"Row {i}\" color=\"#FFCCCCCC\"/>\n      <Button id=\"b{i}\" text=\"Go {i}\" w=\"40\" h=\"14\"/>\n    </Row>\n"
        ));
    }
    s.push_str("  </Column>\n</Screen>\n");
    s
}

fn bench(c: &mut Criterion) {
    let fixtures = [
        ("yui_vm/spellcasting", common::fixture("hexmod/src/ui/spellcasting.yui")),
        ("yui_vm/mod_list", common::fixture("modlist/src/mod_list.yui")),
        ("yui_component/pill", common::fixture("modlist/src/pill.yui")),
        ("yui_component/stat_bar", common::fixture("modlist/src/stat_bar.yui")),
    ];
    for (name, path) in &fixtures {
        let src = common::read(path);
        let stem = path.file_stem().unwrap().to_str().unwrap();
        c.bench_function(&format!("parse/{name}"), |b| {
            b.iter(|| yui::parse_xml(black_box(&src)).unwrap())
        });
        c.bench_function(&format!("generate/{name}"), |b| {
            b.iter(|| yui::generate_for(black_box(&src), stem, Some("m")).unwrap())
        });
    }

    let grid = common::read(&common::fixture("modlist/src/mod_list.yui"));
    let parsed = yui::parse_xml(&grid).unwrap();
    c.bench_function("yui_grid/normalize/mod_list", |b| {
        b.iter(|| {
            let mut root = parsed.clone();
            yui_grid::normalize(black_box(&mut root)).unwrap();
            root
        })
    });

    let big = big_screen(500);
    c.bench_function("parse/yui/big_500", |b| {
        b.iter(|| yui::parse_xml(black_box(&big)).unwrap())
    });
    c.bench_function("generate/yui/big_500", |b| {
        b.iter(|| yui::generate_for(black_box(&big), "big", Some("m")).unwrap())
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
