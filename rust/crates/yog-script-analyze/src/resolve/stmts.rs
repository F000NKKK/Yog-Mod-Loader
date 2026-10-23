//! Statements: what a block binds, what a loop allows, what may be assigned.

use yog_ids::script_problem;

use super::{Context, DefKind, Resolver};
use crate::ast::{Binding, Block, Expr, ExprKind, Stmt};

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

    /// A statement standing as a branch or a body: its own scope.
    fn branch(&mut self, s: &Stmt) {
        self.scoped(|r| r.stmt(s));
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Local { ty, binding, value } => {
                self.opt_ty(ty.as_ref());
                self.opt_expr(value.as_ref());
                self.binding(binding, DefKind::Variable);
            }
            Stmt::Assign { place, value, .. } => {
                self.assigned(place);
                self.expr(value);
            }
            // An `is` pattern's names reach the branch it guards.
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                self.scoped(|r| {
                    r.expr(cond);
                    r.branch(then);
                });
                if let Some(o) = otherwise {
                    self.branch(o);
                }
            }
            Stmt::Foreach {
                ty,
                binding,
                iter,
                body,
            } => {
                self.opt_ty(ty.as_ref());
                self.expr(iter);
                self.looping(|r| {
                    r.binding(binding, DefKind::ReadOnly);
                    r.stmt(body);
                });
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
            } => self.scoped(|r| {
                if let Some(i) = init {
                    r.stmt(i);
                }
                r.opt_expr(cond.as_ref());
                if let Some(s) = step {
                    r.stmt(s);
                }
                r.looping(|r| r.stmt(body));
            }),
            Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => self.scoped(|r| {
                r.expr(cond);
                r.looping(|r| r.stmt(body));
            }),
            Stmt::Block(b) => self.block(b),
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
            Stmt::Expr(e) | Stmt::Value(e) => self.expr(e),
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

    /// The names a local declaration or a `foreach` binds.
    fn binding(&mut self, b: &Binding, kind: DefKind) {
        match b {
            Binding::One(n) => self.bind(n, kind),
            Binding::Tuple(names) => names.iter().for_each(|n| self.bind(n, kind)),
        }
    }

    /// The place an assignment writes: a name of the source's own must be
    /// assignable (members and the game's are checked with their types).
    fn assigned(&mut self, place: &Expr) {
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
                format!(
                    "`{name}` can't be assigned: it's a constant, read-only, a method or a type"
                ),
            );
        }
    }
}
