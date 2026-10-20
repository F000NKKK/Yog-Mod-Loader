//! The runtime's own work on its hot paths, without a game: dispatching an
//! event over mods' handlers, a scheduler tick.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use yog_runtime::bench_support;

fn dispatch(c: &mut Criterion) {
    let mut g = c.benchmark_group("dispatch/bus_event");
    for n in [1, 10, 100] {
        let mut run = bench_support::dispatch(n);
        g.bench_function(BenchmarkId::from_parameter(n), |b| b.iter(&mut run));
    }
    g.finish();
    let mut g = c.benchmark_group("scheduler/tick");
    for n in [10, 1000] {
        let mut run = bench_support::scheduler(n);
        g.bench_function(BenchmarkId::from_parameter(n), |b| b.iter(&mut run));
    }
    g.finish();
}

criterion_group!(benches, dispatch);
criterion_main!(benches);
