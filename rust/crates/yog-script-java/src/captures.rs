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
    /// The method's own locals (not a lambda's).
    declared: HashSet<String>,
    captured: HashSet<String>,
    changed: HashSet<String>,
    /// The names each lambda being gone through declares (its parameters,
    /// its locals), innermost last: one of those isn't a capture.
    lambdas: Vec<HashSet<String>>,
}

impl Seen {
    /// `name` declared where the walk is: the method's, or the lambda's.
    fn declare(&mut self, name: &str) {
        match self.lambdas.last_mut() {
            Some(own) => {
                own.insert(name.to_string());
            }
            None => {
                self.declared.insert(name.to_string());
            }
        }
    }

    /// Whether `name` is a lambda's own, there.
    fn lambdas_own(&self, name: &str) -> bool {
        self.lambdas.iter().any(|own| own.contains(name))
    }
}

impl Seen {
    /// A local function's code: reaching the block's locals as a lambda's does.
    fn local_function(&mut self, d: &yog_script_analyze::ast::Decl) {
        let yog_script_analyze::ast::DeclKind::Method(m) = &d.kind else {
            return;
        };
        self.lambdas
            .push(m.params.iter().map(|p| p.name.text.clone()).collect());
        match &m.body {
            Some(yog_script_analyze::ast::MethodBody::Expr(x)) => self.expr(x, true),
            Some(yog_script_analyze::ast::MethodBody::Block(b)) => {
                b.stmts.iter().for_each(|s| self.stmt(s, true));
            }
            None => {}
        }
        self.lambdas.pop();
    }

    /// A `switch` statement: its value, its guards, its sections.
    fn switch(&mut self, expr: &Expr, sections: &[yog_script_analyze::ast::Section], inside: bool) {
        self.expr(expr, inside);
        for s in sections {
            for (_, guard) in s.labels.iter().flatten() {
                if let Some(g) = guard {
                    self.expr(g, inside);
                }
            }
            s.body.iter().for_each(|x| self.stmt(x, inside));
        }
    }

    fn stmt(&mut self, s: &Stmt, inside: bool) {
        match s {
            Stmt::Function(d) => self.local_function(d),
            Stmt::Switch { expr, sections } => self.switch(expr, sections, inside),
            Stmt::Local { binding, value, .. } => {
                match binding {
                    Binding::One(n) => self.declare(&n.text),
                    Binding::Tuple(names) => names.iter().for_each(|n| self.declare(&n.text)),
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
            Stmt::Foreach {
                binding,
                iter,
                body,
                ..
            } => {
                match binding {
                    Binding::One(n) => self.declare(&n.text),
                    Binding::Tuple(names) => names.iter().for_each(|n| self.declare(&n.text)),
                }
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
            Stmt::Block(b) | Stmt::Unsafe(b, _) => {
                b.stmts.iter().for_each(|s| self.stmt(s, inside))
            }
            Stmt::Lock { target, body, .. } => {
                self.expr(target, inside);
                self.stmt(body, inside);
            }
            Stmt::Return(v, _) | Stmt::Skip(v, _) | Stmt::Yield(v, _) => {
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
            ExprKind::Ident(n) if inside && !self.lambdas_own(n) => {
                self.captured.insert(n.clone());
            }
            ExprKind::Lambda { params, body, .. } => {
                self.lambdas
                    .push(params.iter().map(|p| p.name.text.clone()).collect());
                match body {
                    LambdaBody::Expr(x) => self.expr(x, true),
                    LambdaBody::Block(stmts) => stmts.iter().for_each(|s| self.stmt(s, true)),
                }
                self.lambdas.pop();
            }
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
        if let ExprKind::Ident(n) = &place.kind
            && !self.lambdas_own(n)
        {
            self.changed.insert(n.clone());
        }
    }
}
