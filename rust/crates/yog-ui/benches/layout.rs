//! Flex and grid layout of realistic widget trees.

mod support;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box;
use support::{SCREEN_H, SCREEN_W, SIZES};
use yog_ui::layout;

fn flex(c: &mut Criterion) {
    let mut g = c.benchmark_group("layout/screen");
    for n in SIZES {
        let tree = support::screen(n);
        g.bench_with_input(BenchmarkId::from_parameter(n), &tree, |b, t| {
            b.iter(|| layout::compute(black_box(t), SCREEN_W, SCREEN_H))
        });
    }
    g.finish();

    let mut g = c.benchmark_group("layout/nested");
    // 15, 127 and 1023 nodes.
    for depth in [3, 6, 9] {
        let tree = support::nested(depth);
        g.bench_with_input(BenchmarkId::from_parameter(depth), &tree, |b, t| {
            b.iter(|| layout::compute(black_box(t), SCREEN_W, SCREEN_H))
        });
    }
    g.finish();
}

fn grid(c: &mut Criterion) {
    let mut g = c.benchmark_group("layout/grid");
    for n in SIZES {
        let tree = support::grid(n);
        g.bench_with_input(BenchmarkId::from_parameter(n), &tree, |b, t| {
            b.iter(|| layout::compute(black_box(t), SCREEN_W, SCREEN_H))
        });
    }
    g.finish();
}

criterion_group!(benches, flex, grid);
criterion_main!(benches);
