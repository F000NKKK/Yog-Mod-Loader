//! `yog-script test <file>...`: each source checked, then its `test` blocks
//! run by the interpreter — each passed, or failed with its panic and line.

use yog_script_analyze::system;
use yog_script_run::{Program, run_tests};

pub(super) const WORDS: &[&str] = &["test"];
const USAGE: &str = "usage: yog-script test <file.ys>...";

pub(super) fn run(rest: &[String]) -> Result<(), String> {
    if rest.is_empty() {
        return Err(USAGE.into());
    }
    let mut failed = 0;
    for file in rest {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
        failed += source(file, &text);
    }
    match failed {
        0 => Ok(()),
        n => Err(format!("{n} test(s) failed or didn't build")),
    }
}

/// One source's tests run: how many failed (its errors count as one).
fn source(file: &str, text: &str) -> usize {
    let problems = yog_script_build::check_source(file, text);
    problems.iter().for_each(|d| eprintln!("{d}"));
    if problems.iter().any(yog_data::Diagnostic::is_error) {
        return 1;
    }
    let checked = yog_script_analyze::check(file, text);
    let program = Program::of(&[&system::DECLS, &checked.file.decls]);
    let mut failed = 0;
    for t in run_tests(&program, &checked.file.decls) {
        match t.outcome {
            Ok(()) => println!("test {} ... ok", t.name),
            Err(p) => {
                failed += 1;
                let line = yog_data::LineCol::of(text, p.span.start as usize).line;
                println!("test {} ... FAILED: {} ({file}:{line})", t.name, p.message);
            }
        }
    }
    failed
}
