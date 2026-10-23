//! Statements: locals typed by what's written or their value, what's
//! assigned fitting the place, conditions `bool`, `return` the method's type.

use super::Checker;
use super::model::Type;
use crate::ast::{self, Binding, Block, Expr, ExprKind, Pattern, Stmt};

impl Checker<'_, '_, '_> {
    pub(super) fn block(&mut self, b: &Block) {
        self.scoped(|c| c.stmts(&b.stmts));
    }

    pub(super) fn stmts(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Local { ty, binding, value } => {
                self.local_decl(ty.as_ref(), binding, value.as_ref())
            }
            Stmt::Assign { place, op, value } => {
                let t = self.expr(place);
                if op.is_none() {
                    self.fits(value, &t);
                } else {
                    self.expr(value);
                }
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                self.scoped(|c| {
                    c.patterns_in(cond);
                    c.condition(cond);
                    c.branch(then);
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
            } => self.foreach(ty.as_ref(), binding, iter, body),
            Stmt::For {
                init,
                cond,
                step,
                body,
            } => self.scoped(|c| {
                if let Some(i) = init {
                    c.stmt(i);
                }
                if let Some(cond) = cond {
                    c.condition(cond);
                }
                if let Some(s) = step {
                    c.stmt(s);
                }
                c.branch(body);
            }),
            Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => self.scoped(|c| {
                c.condition(cond);
                c.branch(body);
            }),
            Stmt::Block(b) => self.block(b),
            Stmt::Return(Some(v), _) => match self.returns.clone() {
                Some(r) if r != Type::Void => self.fits(v, &r),
                _ => {
                    self.expr(v);
                }
            },
            Stmt::Skip(Some(v), _) | Stmt::Expr(v) | Stmt::Value(v) => {
                self.expr(v);
            }
            Stmt::Return(None, _) | Stmt::Skip(None, _) | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    /// `T x = v;` / `var x = v;`: the value fits the type written, or gives it.
    fn local_decl(&mut self, ty: Option<&ast::Type>, binding: &Binding, value: Option<&Expr>) {
        let declared = ty.map(|t| self.type_of(t));
        let t = match (declared, value) {
            (Some(d), Some(v)) => {
                self.fits(v, &d);
                d
            }
            (Some(d), None) => d,
            (None, Some(v)) => self.expr(v),
            (None, None) => Type::Unknown,
        };
        self.bind_binding(binding, &t);
    }

    /// `foreach (T x in items)`: `x` an item of the list, or of the type written.
    fn foreach(&mut self, ty: Option<&ast::Type>, binding: &Binding, iter: &Expr, body: &Stmt) {
        let item = match self.expr(iter) {
            Type::List(item) => *item,
            _ => Type::Unknown,
        };
        let t = ty.map_or(item, |t| self.type_of(t));
        self.scoped(|c| {
            c.bind_binding(binding, &t);
            c.branch(body);
        });
    }

    fn branch(&mut self, s: &Stmt) {
        self.scoped(|c| c.stmt(s));
    }

    fn bind_binding(&mut self, b: &Binding, t: &Type) {
        match (b, t) {
            (Binding::One(n), _) => self.bind(&n.text, t.clone()),
            (Binding::Tuple(names), Type::Tuple(items)) if names.len() == items.len() => {
                for (n, item) in names.iter().zip(items) {
                    self.bind(&n.text, item.clone());
                }
            }
            (Binding::Tuple(names), _) => {
                names.iter().for_each(|n| self.bind(&n.text, Type::Unknown))
            }
        }
    }

    /// The names `is` patterns in a condition bind, typed, for the branch
    /// it guards (`if (target is Player p && …)`).
    fn patterns_in(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Is { pattern, .. } => self.bind_pattern(pattern),
            ExprKind::Binary { left, right, .. } => {
                self.patterns_in(left);
                self.patterns_in(right);
            }
            _ => {}
        }
    }

    pub(super) fn bind_pattern(&mut self, p: &Pattern) {
        match p {
            Pattern::Type { ty, bind: Some(n) } => {
                let t = self.type_of(ty);
                self.bind(&n.text, t);
            }
            Pattern::Var(n) => self.bind(&n.text, Type::Unknown),
            Pattern::And(a, b) | Pattern::Or(a, b) => {
                self.bind_pattern(a);
                self.bind_pattern(b);
            }
            _ => {}
        }
    }
}
