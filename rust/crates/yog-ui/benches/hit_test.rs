//! Pointer hit-testing on a laid-out screen.

mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use support::{SCREEN_H, SCREEN_W, SIZES};
use yog_ui::layout;

/// A grid of probe points over the whole screen.
fn probes() -> Vec<(f32, f32)> {
    (0..16)
        .flat_map(|i| (0..9).map(move |j| (i as f32 * 40.0 + 3.0, j as f32 * 40.0 + 3.0)))
        .collect()
}

fn hit(c: &mut Criterion) {
    let points = probes();
    let mut g = c.benchmark_group("hit_test/screen");
    for n in SIZES {
        let node = layout::compute(&support::screen(n), SCREEN_W, SCREEN_H);
        g.bench_with_input(BenchmarkId::from_parameter(n), &node, |b, node| {
            b.iter(|| {
                points
                    .iter()
                    .filter(|&&(x, y)| layout::hit_test(black_box(node), x, y).is_some())
                    .count()
            })
        });
    }
    g.finish();

    let mut g = c.benchmark_group("hit_path/screen");
    for n in SIZES {
        let node = layout::compute(&support::screen(n), SCREEN_W, SCREEN_H);
        g.bench_with_input(BenchmarkId::from_parameter(n), &node, |b, node| {
            b.iter(|| {
                points
                    .iter()
                    .filter_map(|&(x, y)| layout::hit_path(black_box(node), x, y))
                    .count()
            })
        });
    }
    g.finish();
}

criterion_group!(benches, hit);
criterion_main!(benches);
