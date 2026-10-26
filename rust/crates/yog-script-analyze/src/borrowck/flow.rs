//! Statements: locals declared, values assigned anew, and paths — the
//! branches of an `if` joined, a loop's body read twice (what one turn
//! moves, the next can't use), a `return` ending its path.

use super::Checker;
use super::state::Var;
use crate::ast::{Binding, Block, ExprKind, Stmt};

impl Checker<'_, '_> {
    pub(super) fn block(&mut self, b: &Block) {
        self.scoped(|c| b.stmts.iter().for_each(|s| c.stmt(s)));
    }

    fn scoped(&mut self, f: impl FnOnce(&mut Self)) {
        self.state.push();
        f(self);
        self.state.pop();
    }

    pub(super) fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Local { binding, value, .. } => match value {
                Some(v) => {
                    self.expr(v);
                    self.bind(binding);
                }
                None => self.bind_unset(binding),
            },
            Stmt::Assign { place, op, value } => {
                self.expr(value);
                match (&place.kind, op) {
                    // `x = …`: `x` holds a value again.
                    (ExprKind::Ident(name), None) => self.renew(name),
                    _ => self.expr(place),
                }
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                self.expr(cond);
                let before = self.state.clone();
                self.scoped(|c| c.stmt(then));
                let after_then = std::mem::replace(&mut self.state, before);
                if let Some(o) = otherwise {
                    self.scoped(|c| c.stmt(o));
                }
                self.state = after_then.join(self.state.clone());
            }
            Stmt::Foreach {
                binding,
                iter,
                body,
                ..
            } => {
                self.expr(iter);
                self.looping(|c| {
                    c.bind(binding);
                    c.stmt(body);
                });
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
            } => self.scoped(|c| {
                if let Some(i) = init {
                    c.stmt(i);
                }
                c.looping(|c| {
                    c.opt_expr(cond.as_ref());
                    c.stmt(body);
                    if let Some(s) = step {
                        c.stmt(s);
                    }
                });
            }),
            Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => self.looping(|c| {
                c.expr(cond);
                c.stmt(body);
            }),
            _ => self.leaf(s),
        }
    }

    /// The statements with no paths of their own.
    fn leaf(&mut self, s: &Stmt) {
        match s {
            Stmt::Block(b) | Stmt::Unsafe(b, _) => self.block(b),
            Stmt::Return(value, at) | Stmt::Skip(value, at) => {
                self.opt_expr(value.as_ref());
                self.ended(*at);
                self.state.dead = true;
            }
            Stmt::Expr(e) | Stmt::Value(e) => self.expr(e),
            Stmt::Function(d) => self.local_function(d),
            _ => {}
        }
    }

    /// A local function's code: checked on its own, as a lambda's is —
    /// what it moves isn't moved here, its `return` isn't the method's.
    fn local_function(&mut self, d: &crate::ast::Decl) {
        let crate::ast::DeclKind::Method(m) = &d.kind else {
            return;
        };
        let outer = self.state.clone();
        let outs = std::mem::take(&mut self.outs);
        self.state.push();
        for p in &m.params {
            self.state
                .declare(&p.name.text, super::state::Var::default());
        }
        match &m.body {
            Some(crate::ast::MethodBody::Block(b)) => self.block(b),
            Some(crate::ast::MethodBody::Expr(e)) => self.expr(e),
            None => {}
        }
        self.state = outer;
        self.outs = outs;
    }

    /// A loop's body, read twice: the second turn sees what the first moved.
    fn looping(&mut self, mut body: impl FnMut(&mut Self)) {
        let before = self.state.clone();
        for _ in 0..LOOP_READS {
            self.scoped(&mut body);
            self.state = before.clone().join(self.state.clone());
        }
    }

    pub(super) fn bind(&mut self, b: &Binding) {
        match b {
            Binding::One(n) => self.state.declare(&n.text, Var::default()),
            Binding::Tuple(names) => names
                .iter()
                .for_each(|n| self.state.declare(&n.text, Var::default())),
        }
    }

    /// `T x;`: names with no value yet.
    fn bind_unset(&mut self, b: &Binding) {
        match b {
            Binding::One(n) => self.state.declare(&n.text, Var::unset(n.span)),
            Binding::Tuple(names) => names
                .iter()
                .for_each(|n| self.state.declare(&n.text, Var::unset(n.span))),
        }
    }

    /// `x = …`: what `x` was moved from, or declared without, holds a value.
    pub(super) fn renew(&mut self, name: &str) {
        if let Some(v) = self.state.get_mut(name) {
            v.moved = None;
            v.unset = None;
        }
    }
}

/// How often a loop's body is read: once as the first turn, once as any
/// later one.
const LOOP_READS: usize = 2;
