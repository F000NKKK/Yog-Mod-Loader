//! A local a lambda captures and something changes, as C#'s compiler
//! keeps one: in a cell (the runtime's `Ref<T>`, `x.value`) both the code
//! and the lambda reach — Java's lambdas capture only what never changes.

use std::collections::HashSet;

use yog_script_analyze::ast::{Binding, Expr, ExprKind, LambdaBody, Pass, Stmt};

/// The locals of `stmts` (a method's body) a lambda captures and that are
/// changed: by name, anywhere in it.
pub(crate) fn boxed_locals(stmts: &[Stmt]) -> HashSet<String> {
    let mut seen = Seen::default();
    stmts.iter().for_each(|s| seen.stmt(s, false));
    seen.declared
        .into_iter()
        .filter(|n| seen.captured.contains(n) && seen.changed.contains(n))
        .collect()
}

#[derive(Default)]
struct Seen {
    declared: HashSet<String>,
    captured: HashSet<String>,
    changed: HashSet<String>,
}

impl Seen {
    fn stmt(&mut self, s: &Stmt, inside: bool) {
        match s {
            Stmt::Local { binding, value, .. } => {
                match binding {
                    Binding::One(n) => {
                        self.declared.insert(n.text.clone());
                    }
                    Binding::Tuple(names) => names.iter().for_each(|n| {
                        self.declared.insert(n.text.clone());
                    }),
                }
                if let Some(v) = value {
                    self.expr(v, inside);
                }
            }
            Stmt::Assign { place, value, .. } => {
                self.changed_place(place);
                self.expr(place, inside);
                self.expr(value, inside);
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                self.expr(cond, inside);
                self.stmt(then, inside);
                if let Some(o) = otherwise {
                    self.stmt(o, inside);
                }
            }
            Stmt::Foreach { iter, body, .. } => {
                self.expr(iter, inside);
                self.stmt(body, inside);
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
            } => {
                for s in [init, step].into_iter().flatten() {
                    self.stmt(s, inside);
                }
                if let Some(c) = cond {
                    self.expr(c, inside);
                }
                self.stmt(body, inside);
            }
            Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => {
                self.expr(cond, inside);
                self.stmt(body, inside);
            }
            Stmt::Block(b) => b.stmts.iter().for_each(|s| self.stmt(s, inside)),
            Stmt::Return(v, _) | Stmt::Skip(v, _) => {
                if let Some(v) = v {
                    self.expr(v, inside);
                }
            }
            Stmt::Expr(e) | Stmt::Value(e) => self.expr(e, inside),
            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    /// `e`, `inside` a lambda (what it names captured).
    fn expr(&mut self, e: &Expr, inside: bool) {
        match &e.kind {
            ExprKind::Ident(n) if inside => {
                self.captured.insert(n.clone());
            }
            ExprKind::Lambda { body, .. } => match body {
                LambdaBody::Expr(x) => self.expr(x, true),
                LambdaBody::Block(stmts) => stmts.iter().for_each(|s| self.stmt(s, true)),
            },
            ExprKind::Unary { op, expr } if op.steps() => {
                self.changed_place(expr);
                self.expr(expr, inside);
            }
            ExprKind::Call { args, .. }
            | ExprKind::New {
                args: Some(args), ..
            } => {
                for a in args {
                    if matches!(a.pass, Some(Pass::Ref | Pass::Out)) {
                        self.changed_place(&a.value);
                    }
                }
                e.each_child(|c| self.expr(c, inside));
            }
            _ => e.each_child(|c| self.expr(c, inside)),
        }
    }

    /// `place` written: a local's name changed.
    fn changed_place(&mut self, place: &Expr) {
        if let ExprKind::Ident(n) = &place.kind {
            self.changed.insert(n.clone());
        }
    }
}
