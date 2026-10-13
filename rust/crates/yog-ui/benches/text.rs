//! Text measurement and wrapping (Minecraft default-font advance table).

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use yog_ui::text;

const PARAGRAPH: &str = "The quick brown fox jumps over the lazy dog. Patchouli-style books \
    wrap long paragraphs like this one into narrow pages, so every layout pass measures \
    and wraps them again — a supercalifragilisticexpialidociouslylongword included.";

fn text(c: &mut Criterion) {
    c.bench_function("text/str_width", |b| {
        b.iter(|| text::str_width(black_box(PARAGRAPH), 1.0))
    });
    c.bench_function("text/wrap_116px", |b| {
        b.iter(|| text::wrap_text(black_box(PARAGRAPH), 116.0, 1.0))
    });
    c.bench_function("text/height_116px", |b| {
        b.iter(|| text::text_height(black_box(PARAGRAPH), 116.0, 1.0))
    });
    c.bench_function("text/paginate", |b| {
        b.iter(|| text::paginate_text(black_box(PARAGRAPH), 116.0, 40.0, 1.0))
    });
}

criterion_group!(benches, text);
criterion_main!(benches);
