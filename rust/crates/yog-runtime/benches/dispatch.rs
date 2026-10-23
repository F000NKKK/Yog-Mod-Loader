//! The runtime's own work on its hot path, without a game: dispatching an
//! event over mods' handlers.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use yog_runtime::testing::bench_support;

fn dispatch(c: &mut Criterion) {
    let mut g = c.benchmark_group("dispatch/bus_event");
    for n in [1, 10, 100] {
        let mut run = bench_support::dispatch(n);
        g.bench_function(BenchmarkId::from_parameter(n), |b| b.iter(&mut run));
    }
    g.finish();
}

criterion_group!(benches, dispatch);
criterion_main!(benches);
