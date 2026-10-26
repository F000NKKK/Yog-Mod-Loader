//! A mod's Yog Script checked as one project (`yog check`, an editor's
//! check, `yog build`): every source's declarations read once, then each
//! source checked with the others' — for a small, a medium and a large
//! mod. Each source declares a record and a method reaching the previous
//! source's.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use yog_script_analyze::ast::Decl;
use yog_script_analyze::{Env, check_in, declarations};

/// How many sources the mod has.
const SOURCES: [usize; 3] = [4, 32, 128];

fn source(i: usize) -> (String, String) {
    let reach = if i == 0 {
        String::new()
    } else {
        format!("i32 Reach{i}(Part{} p) => p.Size + 1;\n", i - 1)
    };
    let text = format!(
        "public record Part{i}(i32 Size, string Name) {{\n    public i32 Twice() => Size * 2;\n}}\n{reach}"
    );
    (format!("src/part{i}.ys"), text)
}

fn project(c: &mut Criterion) {
    let mut group = c.benchmark_group("project");
    for n in SOURCES {
        let sources: Vec<(String, String)> = (0..n).map(source).collect();
        group.bench_with_input(BenchmarkId::new("declarations", n), &sources, |b, s| {
            b.iter(|| {
                for (name, text) in s {
                    black_box(declarations(name, text, Env::default()));
                }
            });
        });
        let decls: Vec<Vec<Decl>> = sources
            .iter()
            .map(|(name, text)| declarations(name, text, Env::default()))
            .collect();
        group.bench_with_input(BenchmarkId::new("check_all", n), &sources, |b, s| {
            b.iter(|| {
                for (i, (name, text)) in s.iter().enumerate() {
                    let others: Vec<&[Decl]> = decls
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| *j != i)
                        .map(|(_, d)| d.as_slice())
                        .collect();
                    let env = Env {
                        others: &others,
                        ..Env::default()
                    };
                    black_box(check_in(name, text, env));
                }
            });
        });
    }
    group.finish();
}

criterion_group!(benches, project);
criterion_main!(benches);
