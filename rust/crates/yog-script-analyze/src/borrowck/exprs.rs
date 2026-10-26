//! Expressions: names used, `move`s, a call's arguments lent together,
//! lambdas' captures, names patterns and `out let` bind.

use yog_ids::script_problem;

use super::Checker;
use super::state::Var;
use crate::ast::{Arg, Expr, ExprKind, LambdaBody, Pass};
use crate::source::token::Span;

impl Checker<'_, '_> {
    pub(super) fn opt_expr(&mut self, e: Option<&Expr>) {
        if let Some(e) = e {
            self.expr(e);
        }
    }

    pub(super) fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Ident(name) => self.used(name, e.span),
            ExprKind::Move(inner) => self.moved(inner, e.span),
            ExprKind::Declare { name, .. } => self.state.declare(&name.text, Var::default()),
            ExprKind::Call { callee, args } => {
                self.expr(callee);
                self.args(args);
            }
            ExprKind::New { args, init, .. } => {
                self.args(args.as_deref().unwrap_or_default());
                init.iter().for_each(|(_, v)| self.expr(v));
            }
            ExprKind::Lambda { params, body, .. } => {
                // Its code runs later: what it moves isn't moved here, its
                // `return` isn't the method's.
                let outer = self.state.clone();
                let outs = std::mem::take(&mut self.outs);
                self.state.push();
                for p in params {
                    self.state.declare(&p.name.text, Var::default());
                }
                match body {
                    LambdaBody::Expr(e) => self.expr(e),
                    LambdaBody::Block(stmts) => stmts.iter().for_each(|s| self.stmt(s)),
                }
                self.state = outer;
                self.outs = outs;
            }
            ExprKind::Is { expr, pattern } => {
                self.expr(expr);
                pattern.each_binding(&mut |n| self.state.declare(&n.text, Var::default()));
            }
            _ => e.each_child(|c| self.expr(c)),
        }
    }

    /// A name read: given a value, and not after a `move`, on every path to here.
    fn used(&mut self, name: &str, at: Span) {
        if self.state.get(name).is_some_and(|v| v.unset.is_some()) {
            self.problem(
                script_problem::UNASSIGNED,
                at,
                format!("`{name}` is read before it's given a value on some path to here"),
            );
            return;
        }
        if self.state.get(name).is_some_and(|v| v.moved.is_some()) {
            self.problem(
                script_problem::MOVED,
                at,
                format!(
                    "`{name}` was handed on by `move` before this: give it a value again, \
                     `Clone()` it before the move, or lend it (`in {name}`) instead"
                ),
            );
        }
    }

    /// `move x`: `x` usable here, gone after; a borrowed `x` isn't ours.
    fn moved(&mut self, inner: &Expr, at: Span) {
        let ExprKind::Ident(name) = &inner.kind else {
            return self.expr(inner);
        };
        self.used(name, inner.span);
        let Some(var) = self.state.get_mut(name) else {
            return;
        };
        if var.lent.is_some() {
            self.problem(
                script_problem::BORROW,
                at,
                format!("`{name}` is lent to this method (`in`/`ref`): it can't be moved away"),
            );
            return;
        }
        var.moved = Some(at);
    }

    /// A call's arguments: one `ref` or `move` of a value and nothing else
    /// of it in the same call; then each, in order.
    fn args(&mut self, args: &[Arg]) {
        for (i, a) in args.iter().enumerate() {
            let (Some(pass @ (Pass::Ref | Pass::Move)), ExprKind::Ident(name)) =
                (a.pass, &a.value.kind)
            else {
                continue;
            };
            let again = args
                .iter()
                .enumerate()
                .any(|(j, b)| j != i && matches!(&b.value.kind, ExprKind::Ident(n) if n == name));
            if again {
                let how = if pass == Pass::Ref {
                    "lent by `ref`"
                } else {
                    "moved"
                };
                self.problem(
                    script_problem::BORROW,
                    a.value.span,
                    format!("`{name}` is {how} and passed again in the same call"),
                );
            }
        }
        for a in args {
            match (a.pass, &a.value.kind) {
                (Some(Pass::Move), _) => self.moved(&a.value, a.value.span),
                (Some(Pass::Out), ExprKind::Ident(name)) => self.renew(name),
                _ => self.expr(&a.value),
            }
        }
    }
}
