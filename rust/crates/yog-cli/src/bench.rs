//! `yog bench` — a mod's benchmarks: `benches/*.rs` files whose
//! `#[yog_api::bench]` functions run under criterion.
//!
//! ```ignore
//! // benches/casting.rs
//! use criterion::{Bencher, Criterion};
//!
//! #[yog_api::bench]
//! fn parse(b: &mut Bencher) {
//!     b.iter(|| hexcasting::math::HexPattern::from_angles("qaq", HexDir::East));
//! }
//!
//! #[yog_api::bench]
//! fn vm(c: &mut Criterion) {
//!     let mut group = c.benchmark_group("vm");
//!     // ...
//! }
//! ```
//!
//! No harness, no `criterion_main!`, no Cargo.toml: each file becomes a
//! `[[bench]]` target of the generated crate whose `main`
//! (`.yog-build/generated/benches/<file>.rs`) runs its functions in source
//! order. A function taking `&mut Bencher` is one benchmark named
//! `<file>/<fn>`; one taking `&mut Criterion` sets up its own (groups,
//! throughput, batched inputs). The mod is linked in as a library under
//! its crate name; with no game, the bus's actions answer nothing
//! (their replies' defaults).

use std::path::Path;
use yog_data::Diagnostic;

use crate::manifest::YogToml;

/// Where the mod's benchmark files live, under the project root.
pub const BENCH_DIR: &str = "benches";
/// The generated `main`s, under the project root.
pub const GENERATED_DIR: &str = ".yog-build/generated/benches";
/// criterion, added to `[dev-dependencies]` unless yog.toml lists its own.
const CRITERION: &str = r#"criterion = { version = "0.8", default-features = false, features = ["cargo_bench_support"] }"#;
/// The marker attribute, as written in a benchmark file.
const ATTRIBUTE: &str = "#[yog_api::bench]";

/// What a benchmark function takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Takes {
    /// `&mut Bencher` — one benchmark, `b.iter(...)`.
    Bencher,
    /// `&mut Criterion` — the function registers its own.
    Criterion,
}

/// One `benches/<name>.rs` file and its `#[yog_api::bench]` functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchFile {
    pub name: String,
    pub functions: Vec<(String, Takes)>,
}

/// A problem with a benchmark file.
fn problem(message: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::error(yog_ids::build_problem::BENCH, message)
}

/// The mod's benchmark files, by name; none without a `benches/` folder.
pub fn discover(root: &Path) -> Result<Vec<BenchFile>, Diagnostic> {
    let dir = root.join(BENCH_DIR);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let shown = crate::shown_path(root, &path);
        let source = std::fs::read_to_string(&path).map_err(|e| problem(e).in_file(&shown))?;
        let functions = functions(&source).map_err(|e| e.in_file(&shown))?;
        if !functions.is_empty() {
            files.push(BenchFile {
                name: name.to_string(),
                functions,
            });
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

/// The `#[yog_api::bench]` functions of a benchmark file, in source order.
pub fn functions(source: &str) -> Result<Vec<(String, Takes)>, Diagnostic> {
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find(ATTRIBUTE) {
        let line = source[..source.len() - rest.len() + at]
            .matches('\n')
            .count()
            + 1;
        rest = &rest[at + ATTRIBUTE.len()..];
        let Some(fn_at) = rest.find("fn ") else {
            return Err(problem(format!("{ATTRIBUTE} must be on a function")).at_line(line));
        };
        let after = &rest[fn_at + 3..];
        let name: String = after
            .trim_start()
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let params = after
            .split_once('(')
            .and_then(|(_, p)| p.split_once(')'))
            .map(|(p, _)| p)
            .unwrap_or_default();
        let takes = if params.contains("Criterion") {
            Takes::Criterion
        } else if params.contains("Bencher") {
            Takes::Bencher
        } else {
            return Err(problem(format!(
                "`{name}` must take `&mut Bencher` or `&mut Criterion`"
            ))
            .at_line(line));
        };
        out.push((name, takes));
    }
    Ok(out)
}

/// The generated `main` of one benchmark file.
pub fn main_source(file: &BenchFile) -> String {
    let mut calls = String::new();
    for (function, takes) in &file.functions {
        calls.push_str(&match takes {
            Takes::Bencher => {
                format!(
                    "    c.bench_function(\"{}/{function}\", bench::{function});\n",
                    file.name
                )
            }
            Takes::Criterion => format!("    bench::{function}(&mut c);\n"),
        });
    }
    format!(
        "// Generated by yog from {BENCH_DIR}/{name}.rs — do not edit.\n\
         #[path = \"../../../{BENCH_DIR}/{name}.rs\"]\n\
         mod bench;\n\n\
         fn main() {{\n\
         \x20   let mut c = ::criterion::Criterion::default().configure_from_args();\n\
         {calls}\
         \x20   c.final_summary();\n\
         }}\n",
        name = file.name,
    )
}

/// Writes every benchmark file's `main`, dropping those of removed files.
pub fn write_sources(root: &Path, files: &[BenchFile]) -> Result<(), String> {
    let dir = root.join(GENERATED_DIR);
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let stale = path
                .file_stem()
                .and_then(|s| s.to_str())
                .is_none_or(|stem| !files.iter().any(|f| f.name == stem));
            if stale {
                crate::files::remove(&path);
            }
        }
    }
    for file in files {
        crate::write_if_changed(&dir.join(format!("{}.rs", file.name)), main_source(file))?;
    }
    Ok(())
}

/// The generated Cargo.toml's `[dev-dependencies]` — `test_deps` (the
/// mod's tests' own), and with benchmarks theirs — and `[[bench]]` targets.
pub fn cargo_sections(meta: &YogToml, files: &[BenchFile], test_deps: &[String]) -> String {
    if files.is_empty() && test_deps.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n[dev-dependencies]\n");
    for dep in test_deps {
        out.push_str(dep);
        out.push('\n');
    }
    if files.is_empty() {
        return out;
    }
    for (name, spec) in &meta.dev_dependencies {
        out.push_str(&format!("{name} = {spec}\n"));
    }
    if !meta
        .dev_dependencies
        .iter()
        .any(|(name, _)| name == "criterion")
    {
        out.push_str(CRITERION);
        out.push('\n');
    }
    for file in files {
        out.push_str(&format!(
            "\n[[bench]]\nname    = \"{name}\"\npath    = \"generated/benches/{name}.rs\"\nharness = false\n",
            name = file.name
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_marked_functions_in_order() {
        let src = "use criterion::{Bencher, Criterion};\n\
                   fn helper() {}\n\
                   #[yog_api::bench]\nfn parse(b: &mut Bencher) { b.iter(|| 1) }\n\
                   #[yog_api::bench]\npub fn groups(c: &mut Criterion) {}\n";
        assert_eq!(
            functions(src).unwrap(),
            [
                ("parse".to_string(), Takes::Bencher),
                ("groups".to_string(), Takes::Criterion)
            ]
        );
    }

    #[test]
    fn rejects_a_function_without_a_bencher() {
        let e = functions("\n#[yog_api::bench]\nfn nope(x: u32) {}\n").unwrap_err();
        assert_eq!(e.line, Some(2), "{e}");
    }

    #[test]
    fn main_runs_each_function() {
        let file = BenchFile {
            name: "casting".into(),
            functions: vec![
                ("parse".into(), Takes::Bencher),
                ("vm".into(), Takes::Criterion),
            ],
        };
        let main = main_source(&file);
        assert!(main.contains("#[path = \"../../../benches/casting.rs\"]"));
        assert!(main.contains("c.bench_function(\"casting/parse\", bench::parse);"));
        assert!(main.contains("bench::vm(&mut c);"));
    }
}
