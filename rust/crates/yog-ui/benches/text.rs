//! Text measurement and wrapping: Minecraft's default-font advance table,
//! and a non-default font measured through the (no-op) host.

mod support;

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use yog_gfx::GfxContext;
use yog_ui::text;

const HOST_FONT: &str = "yog:font/uniform";

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

    let table = support::null_gfx();
    // SAFETY: `table` outlives every use: the host metrics only call its
    // `text_width`, a plain function.
    text::use_host_metrics(&unsafe { GfxContext::from_raw(&table) });
    c.bench_function("text/str_width_host_font", |b| {
        b.iter(|| text::str_width_in(black_box(PARAGRAPH), 1.0, HOST_FONT))
    });
    c.bench_function("text/height_116px_host_font", |b| {
        b.iter(|| text::text_height_in(black_box(PARAGRAPH), 116.0, 1.0, HOST_FONT))
    });
}

criterion_group!(benches, text);
criterion_main!(benches);
