//! The borrow checker (`ys.md` §3, Memory): what a method's code does with
//! the values it owns and borrows — a value used after `move` handed it on,
//! a `ref` beside another borrow of the same value in one call, a borrowed
//! parameter moved away, a local read before it has a value, an `out`
//! parameter not given one. Moves and borrows are written at the call
//! (`move x`, `ref x`), so this reads the code, not its types.

mod exprs;
mod flow;
mod state;

use std::collections::HashSet;

use crate::ast::{Block, Decl, DeclKind, File, MethodBody, Param, Pass, accessor_scope};
use crate::source::token::Span;
use state::{State, Var};
use yog_analysis::Report;
use yog_ids::script_problem;

pub(crate) fn check(file: &File, types: &crate::typeck::Types, report: &mut Report<'_>) {
    Checker {
        report,
        types,
        state: State::new(),
        reported: HashSet::new(),
        outs: Vec::new(),
    }
    .decls(&file.decls);
}

struct Checker<'r, 's> {
    report: &'r mut Report<'s>,
    /// The types checked: what a lambda handed to another thread takes.
    types: &'r crate::typeck::Types,
    state: State,
    /// Where a problem was reported already: a loop's body is read twice.
    reported: HashSet<(u32, u32)>,
    /// The method's `out` parameters: each given a value before it ends.
    outs: Vec<String>,
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
        self.ended(body_end(params));
    }

    fn block_body(&mut self, params: &[Param], body: &Block) {
        self.fresh(params);
        self.block(body);
        self.ended(body_end(params));
    }

    /// A method's start: its parameters, nothing moved yet.
    fn fresh(&mut self, params: &[Param]) {
        self.state = State::new();
        self.outs.clear();
        for p in params {
            let var = Var {
                lent: p.pass.filter(|p| matches!(p, Pass::In | Pass::Ref)),
                moved: None,
                unset: (p.pass == Some(Pass::Out)).then_some(p.name.span),
            };
            if p.pass == Some(Pass::Out) {
                self.outs.push(p.name.text.clone());
            }
            self.state.declare(&p.name.text, var);
        }
    }

    /// A path out of the method, at `at`: every `out` parameter given a value.
    fn ended(&mut self, at: Span) {
        if self.state.dead {
            return;
        }
        let unset: Vec<String> = self
            .outs
            .iter()
            .filter(|o| self.state.get(o).is_some_and(|v| v.unset.is_some()))
            .cloned()
            .collect();
        for name in unset {
            self.problem(
                script_problem::UNASSIGNED,
                at,
                format!(
                    "`out` parameter `{name}` isn't given a value on this path out of the method"
                ),
            );
        }
    }

    /// Reports `message` at `at` once, however often the code is read.
    fn problem(&mut self, code: &str, at: Span, message: String) {
        if self.reported.insert((at.start, at.end)) {
            self.report.error(code, at, message);
        }
    }
}

/// Where a method with `params` is reported ending without giving an
/// `out` one a value: the first such parameter's name.
fn body_end(params: &[Param]) -> Span {
    params
        .iter()
        .find(|p| p.pass == Some(Pass::Out))
        .map_or_else(Span::default, |p| p.name.span)
}
