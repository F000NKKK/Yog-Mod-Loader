//! The screen designer's render on a synthetic screen, including the JSON
//! a tool gets.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use yog_cli::design::{RenderArgs, render};

const WORDS: &[&str] = &["project", "window", "state", "screen", "button", "label", "texture", "model", "block", "item"];

fn word(i: usize) -> &'static str {
    WORDS[i % WORDS.len()]
}

fn camel(i: usize) -> String {
    [word(i), word(i / 3 + 1), word(i / 7 + 2)]
        .iter()
        .map(|w| w[..1].to_uppercase() + &w[1..])
        .collect()
}

fn snake(i: usize) -> String {
    format!("{}_{}_{}", word(i), word(i / 5 + 3), word(i / 11 + 4))
}

fn designer(c: &mut Criterion) {
    let label = |i: usize| {
        serde_json::json!({
            "uid": format!("l{i}"), "kind": "Label",
            "attrs": { "text": format!("Row {i}: §e{}", snake(i)), "color": "#FFFFFF" },
            "children": []
        })
    };
    let button = |i: usize| {
        serde_json::json!({
            "uid": format!("b{i}"), "kind": "Button",
            "attrs": { "w": "80", "h": "20", "text": camel(i), "on_click": "go" },
            "children": []
        })
    };
    let column = |from: usize| {
        serde_json::json!({
            "uid": format!("c{from}"), "kind": "Column",
            "attrs": { "spacing": "2" },
            "children": (from..from + 12)
                .map(|i| if i % 3 == 0 { button(i) } else { label(i) })
                .collect::<Vec<_>>()
        })
    };
    let args: RenderArgs = serde_json::from_value(serde_json::json!({
        "path": "/nowhere/src/bench.yui",
        "root": {
            "uid": "s", "kind": "Screen",
            "attrs": { "width": "400", "height": "240", "bg": "#C0101010" },
            "children": [{
                "uid": "row", "kind": "Row",
                "attrs": { "spacing": "8" },
                "children": [column(0), column(12), column(24)]
            }]
        },
        "screenW": 427, "screenH": 240, "scale": 2, "preview": false
    }))
    .unwrap();
    c.bench_function("designer_render/36_widgets", |b| {
        b.iter(|| serde_json::to_string(&render(black_box(&args)).unwrap()).unwrap())
    });
}

criterion_group!(benches, designer);
criterion_main!(benches);
