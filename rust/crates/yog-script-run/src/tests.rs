//! A source's `test "…" { … }` blocks run, each on its own: passed, or the
//! panic that failed it.

use yog_script_analyze::ast::{Decl, DeclKind};

use crate::interp::{Interp, Panic};
use crate::program::Program;

/// One test's end.
pub struct TestRun {
    pub name: String,
    /// `Err`: the panic that failed it.
    pub outcome: Result<(), Panic>,
}

/// Every test of `decls`, run against `program`.
pub fn run_tests(program: &Program<'_>, decls: &[Decl]) -> Vec<TestRun> {
    let mut out = Vec::new();
    collect(program, decls, &mut out);
    out
}

fn collect(program: &Program<'_>, decls: &[Decl], out: &mut Vec<TestRun>) {
    for d in decls {
        match &d.kind {
            DeclKind::Test { name, body } => out.push(TestRun {
                name: name.clone(),
                outcome: Interp::new(program).run_block(body),
            }),
            kind => collect(program, kind.members(), out),
        }
    }
}
