//! Statements: what a block binds, what a loop allows, what may be assigned.

use yog_ids::script_problem;

use super::{Context, DefKind, Resolver};
use crate::ast::{Binding, Block, Expr, ExprKind, Stmt, Type};

impl Resolver<'_, '_> {
    /// A block in a scope of its own.
    pub(super) fn block(&mut self, b: &Block) {
        self.scoped(|r| r.stmts(&b.stmts));
    }

    pub(super) fn stmts(&mut self, stmts: &[Stmt]) {
        // A block's local functions are called anywhere in it, before them too.
        for f in crate::ast::local_functions(stmts) {
            self.bind(&f.name, DefKind::Method);
        }
        for s in stmts {
            self.stmt(s);
        }
    }

    /// A statement standing as a branch or a body: its own scope.
    fn branch(&mut self, s: &Stmt) {
        self.scoped(|r| r.stmt(s));
    }

    pub(super) fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Function(d) => {
                if let crate::ast::DeclKind::Method(m) = &d.kind {
                    self.method(m, d.has(crate::ast::Modifier::Async));
                }
            }
            Stmt::Local {
                ty,
                binding,
                value,
                mutable,
            } => self.local(ty.as_ref(), binding, value.as_ref(), *mutable),
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
                mutable,
                awaits,
            } => self.foreach(ty.as_ref(), (binding, *mutable), (iter, *awaits), body),
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
            Stmt::Block(b) | Stmt::Unsafe(b, _) => self.block(b),
            Stmt::Break(at) | Stmt::Continue(at) if self.cx.loops == 0 => {
                self.report.error(
                    script_problem::OUTSIDE_LOOP,
                    *at,
                    "`break` and `continue` go in a loop",
                );
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
            Stmt::Return(value, _) | Stmt::Yield(value, _) => self.opt_expr(value.as_ref()),
            Stmt::Lock {
                target,
                value,
                body,
                ..
            } => self.lock_stmt(target, value.as_ref(), body),
            Stmt::Skip(value, at) => self.skip(value.as_ref(), *at),
            Stmt::Expr(e) | Stmt::Value(e) => self.expr(e),
        }
    }

    /// `skip value;`: only in a patch's `before`.
    fn skip(&mut self, value: Option<&Expr>, at: crate::source::token::Span) {
        if !self.cx.before {
            self.report.error(
                script_problem::SKIP_OUTSIDE_BEFORE,
                at,
                "`skip` ends the patched method: it goes in a patch's `before`",
            );
        }
        self.opt_expr(value);
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
    /// `let mut x: T = value;`: the value read before `x` is.
    fn local(&mut self, ty: Option<&Type>, b: &Binding, value: Option<&Expr>, mutable: bool) {
        self.opt_ty(ty);
        self.opt_expr(value);
        self.binding(b, DefKind::local(mutable));
    }

    fn binding(&mut self, b: &Binding, kind: DefKind) {
        match b {
            Binding::One(n) => self.bind(n, kind),
            Binding::Tuple(names) => names.iter().for_each(|n| self.bind(n, kind)),
        }
    }

    /// `lock (target, ref let value) body`: no `await` while it's held.
    fn lock_stmt(&mut self, target: &Expr, value: Option<&crate::ast::Name>, body: &Stmt) {
        self.expr(target);
        let outer = std::mem::replace(&mut self.cx.locked, true);
        self.scoped(|r| {
            if let Some(n) = value {
                r.bind(n, DefKind::Variable);
            }
            r.stmt(body);
        });
        self.cx.locked = outer;
    }

    /// `foreach (… in iter) body` (`awaits`: `await foreach`, in an `async` method only).
    fn foreach(
        &mut self,
        ty: Option<&crate::ast::Type>,
        (binding, mutable): (&Binding, bool),
        (iter, awaits): (&Expr, bool),
        body: &Stmt,
    ) {
        self.opt_ty(ty);
        if awaits && !self.cx.awaits {
            self.report.error(
                script_problem::AWAIT_OUTSIDE_ASYNC,
                iter.span,
                "`await foreach` waits in an `async` method",
            );
        }
        self.expr(iter);
        // `let mut`: each item lent to be changed; else read only.
        let kind = if mutable {
            DefKind::Variable
        } else {
            DefKind::ReadOnly
        };
        self.looping(|r| {
            r.binding(binding, kind);
            r.stmt(body);
        });
    }

    /// The place an assignment writes: a name of the source's own must be
    /// assignable (members and the game's are checked with their types).
    pub(super) fn assigned(&mut self, place: &Expr) {
        let ExprKind::Ident(name) = &place.kind else {
            self.expr(place);
            return;
        };
        let constructing = self.cx.constructing;
        if self.refer(name, place.span).is_some_and(|kind| {
            !(kind.assignable() || kind == DefKind::ReadOnlyField && constructing)
        }) {
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
