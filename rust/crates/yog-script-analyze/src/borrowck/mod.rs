//! The borrow checker (`ys.md` §3, Memory): what a method's code does with
//! the values it owns and borrows — a value used after `move` handed it on,
//! a `ref` beside another borrow of the same value in one call, a borrowed
//! parameter moved away. Moves and borrows are written at the call
//! (`move x`, `ref x`), so this reads the code, not its types.

mod exprs;
mod flow;
mod state;

use std::collections::HashSet;

use crate::ast::{Block, Decl, DeclKind, File, MethodBody, Param, Pass, accessor_scope};
use crate::source::token::Span;
use state::{State, Var};
use yog_analysis::Report;

pub(crate) fn check(file: &File, report: &mut Report<'_>) {
    Checker {
        report,
        state: State::new(),
        reported: HashSet::new(),
    }
    .decls(&file.decls);
}

struct Checker<'r, 's> {
    report: &'r mut Report<'s>,
    state: State,
    /// Where a problem was reported already: a loop's body is read twice.
    reported: HashSet<(u32, u32)>,
}

impl Checker<'_, '_> {
    fn decls(&mut self, decls: &[Decl]) {
        for d in decls {
            match &d.kind {
                DeclKind::Method(m) => {
                    if let Some(body) = &m.body {
                        self.body(&m.params, body);
                    }
                }
                DeclKind::Ctor(c) => self.block_body(&c.params, &c.body),
                DeclKind::Operator(op) => {
                    if let Some(body) = &op.method.body {
                        self.body(&op.method.params, body);
                    }
                }
                DeclKind::Property { ty, accessors, .. } => {
                    for a in accessors {
                        if let Some(body) = &a.body {
                            self.body(&accessor_scope(ty, a, true), body);
                        }
                    }
                }
                DeclKind::Indexer(ix) => {
                    for a in &ix.accessors {
                        if let Some(body) = &a.body {
                            let params =
                                [ix.params.clone(), accessor_scope(&ix.ty, a, false)].concat();
                            self.body(&params, body);
                        }
                    }
                }
                DeclKind::System { params, body, .. } => self.block_body(params, body),
                DeclKind::Test { body, .. } => self.block_body(&[], body),
                kind => self.decls(kind.members()),
            }
        }
    }

    fn body(&mut self, params: &[Param], body: &MethodBody) {
        self.fresh(params);
        match body {
            MethodBody::Block(b) => self.block(b),
            MethodBody::Expr(e) => self.expr(e),
        }
    }

    fn block_body(&mut self, params: &[Param], body: &Block) {
        self.fresh(params);
        self.block(body);
    }

    /// A method's start: its parameters, nothing moved yet.
    fn fresh(&mut self, params: &[Param]) {
        self.state = State::new();
        for p in params {
            let var = Var {
                lent: p.pass.filter(|p| matches!(p, Pass::In | Pass::Ref)),
                moved: None,
            };
            self.state.declare(&p.name.text, var);
        }
    }

    /// Reports `message` at `at` once, however often the code is read.
    fn problem(&mut self, code: &str, at: Span, message: String) {
        if self.reported.insert((at.start, at.end)) {
            self.report.error(code, at, message);
        }
    }
}
