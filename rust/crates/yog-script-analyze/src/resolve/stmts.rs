//! Statements and patterns: what a block binds, what a loop allows.

use yog_ids::script_problem;

use super::{Context, DefKind, Resolver};
use crate::ast::{Block, Cond, ExprKind, Name, Pattern, Stmt};

impl Resolver<'_, '_> {
    /// A block in a scope of its own.
    pub(super) fn block(&mut self, b: &Block) {
        self.scoped(|r| r.stmts(&b.stmts));
    }

    pub(super) fn stmts(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Let { pattern, ty, value } => {
                self.opt_ty(ty.as_ref());
                self.expr(value);
                self.bind_pattern(pattern);
            }
            Stmt::Var { name, ty, value } => {
                self.opt_ty(ty.as_ref());
                self.opt_expr(value.as_ref());
                self.bind(name, DefKind::Variable);
            }
            Stmt::Assign { place, value, .. } => {
                self.assigned(place);
                self.expr(value);
            }
            Stmt::For {
                pattern,
                iter,
                body,
            } => {
                self.expr(iter);
                self.looping(|r| {
                    r.bind_pattern(pattern);
                    r.stmts(&body.stmts);
                });
            }
            Stmt::While { cond, body } => self.scoped(|r| {
                r.cond(cond);
                r.looping(|r| r.stmts(&body.stmts));
            }),
            Stmt::Loop(body) => self.looping(|r| r.stmts(&body.stmts)),
            Stmt::Break(at) | Stmt::Continue(at) if self.cx.loops == 0 => {
                self.report.error(
                    script_problem::OUTSIDE_LOOP,
                    *at,
                    "`break` and `continue` go in a loop",
                );
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
            Stmt::Return(value, _) => self.opt_expr(value.as_ref()),
            Stmt::Skip(value, at) => {
                if !self.cx.before {
                    self.report.error(
                        script_problem::SKIP_OUTSIDE_BEFORE,
                        *at,
                        "`skip` ends the patched method: it goes in a patch's `before`",
                    );
                }
                self.opt_expr(value.as_ref());
            }
            Stmt::Expr(e) => self.expr(e),
        }
    }

    /// A loop's body: in a scope, `break` allowed.
    fn looping(&mut self, f: impl FnOnce(&mut Self)) {
        let cx = Context {
            loops: self.cx.loops + 1,
            ..self.cx
        };
        self.within(cx, |r| r.scoped(f));
    }

    /// An `if`'s or a `while`'s condition: `if let` binds for what follows.
    pub(super) fn cond(&mut self, c: &Cond) {
        match c {
            Cond::Expr(e) => self.expr(e),
            Cond::Let { pattern, value } => {
                self.expr(value);
                self.bind_pattern(pattern);
            }
        }
    }

    /// The place an assignment writes: a name of the source's own must be
    /// a variable (members and the game's are checked with their types).
    fn assigned(&mut self, place: &crate::ast::Expr) {
        let ExprKind::Ident(name) = &place.kind else {
            self.expr(place);
            return;
        };
        if self
            .refer(name, place.span)
            .is_some_and(|kind| !kind.assignable())
        {
            self.report.error(
                script_problem::ASSIGN_TO_CONSTANT,
                place.span,
                format!("`{name}` isn't a `var`: it can't be assigned"),
            );
        }
    }

    /// The names a pattern binds, bound here; what it compares with, looked up.
    pub(super) fn bind_pattern(&mut self, p: &Pattern) {
        match p {
            Pattern::Wild(_) | Pattern::Else(_) => {}
            // A single name: a case or a value it compares with when there
            // is one, else a new binding.
            Pattern::Value(e) => match &e.kind {
                ExprKind::Ident(name) if self.lookup(name).is_none() => {
                    self.bind(
                        &Name {
                            text: name.clone(),
                            span: e.span,
                        },
                        DefKind::Value,
                    );
                }
                _ => self.expr(e),
            },
            Pattern::Range { from, to } => {
                self.expr(from);
                self.expr(to);
            }
            Pattern::Is { ty, bind } => {
                self.ty(ty);
                if let Some(n) = bind {
                    self.bind(n, DefKind::Value);
                }
            }
            Pattern::Variant { path, args } => {
                if let Some(first) = path.first() {
                    self.refer(&first.text, first.span);
                }
                args.iter().for_each(|a| self.bind_pattern(a));
            }
            Pattern::Tuple(ps) | Pattern::Or(ps) => ps.iter().for_each(|a| self.bind_pattern(a)),
        }
    }

    /// The names a declaration's pattern declares (`let (a, b) = …`).
    pub(super) fn pattern_names(&mut self, p: &Pattern, again: bool) {
        match p {
            Pattern::Value(e) => {
                if let ExprKind::Ident(name) = &e.kind {
                    let n = Name {
                        text: name.clone(),
                        span: e.span,
                    };
                    self.define(&n, DefKind::Value, again);
                }
            }
            Pattern::Is { bind: Some(n), .. } => self.define(n, DefKind::Value, again),
            Pattern::Variant { args, .. } => args.iter().for_each(|a| self.pattern_names(a, again)),
            Pattern::Tuple(ps) => ps.iter().for_each(|a| self.pattern_names(a, again)),
            _ => {}
        }
    }
}
