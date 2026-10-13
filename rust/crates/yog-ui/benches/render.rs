//! Draw-command generation from a laid-out tree (against a no-op graphics
//! table, so only the UI's own work is measured).

mod support;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use support::{SCREEN_H, SCREEN_W, SIZES};
use yog_gfx::GfxContext;
use yog_ui::UiRoot;

fn render(c: &mut Criterion) {
    let table = support::null_gfx();
    // SAFETY: `table` outlives `ctx` and every function in it is a no-op.
    let ctx = unsafe { GfxContext::from_raw(&table) };
    let mut g = c.benchmark_group("render/screen");
    for n in SIZES {
        let mut ui = UiRoot::new("bench", support::screen(n));
        ui.layout(SCREEN_W, SCREEN_H);
        g.bench_with_input(BenchmarkId::from_parameter(n), &ui, |b, ui| {
            b.iter(|| ui.render(&ctx))
        });
    }
    g.finish();

    let mut g = c.benchmark_group("render/grid");
    for n in SIZES {
        let mut ui = UiRoot::new("bench", support::grid(n));
        ui.layout(SCREEN_W, SCREEN_H);
        g.bench_with_input(BenchmarkId::from_parameter(n), &ui, |b, ui| {
            b.iter(|| ui.render(&ctx))
        });
    }
    g.finish();
}

criterion_group!(benches, render);
criterion_main!(benches);
