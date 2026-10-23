//! Expressions and types: names looked up, `it` and `self` where they mean
//! something, lambdas and match arms in scopes of their own.

use yog_ids::script_problem;

use super::{Context, DefKind, Resolver};
use crate::ast::{Expr, ExprKind, StrSeg, Type, TypeKind};

impl Resolver<'_, '_> {
    pub(super) fn opt_expr(&mut self, e: Option<&Expr>) {
        if let Some(e) = e {
            self.expr(e);
        }
    }

    pub(super) fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Ident(name) => {
                self.refer(name, e.span);
            }
            ExprKind::It if !self.cx.it => self.report.error(
                script_problem::IT_OUTSIDE_LAMBDA,
                e.span,
                "`it` is a lambda's argument: `{ it * 2 }`",
            ),
            ExprKind::SelfValue if !self.cx.member => self.report.error(
                script_problem::SELF_OUTSIDE_MEMBER,
                e.span,
                "`self` is the object in a type's, a content's or a patch's members",
            ),
            ExprKind::Str(segs) => {
                for s in segs {
                    if let StrSeg::Expr(e) = s {
                        self.expr(e);
                    }
                }
            }
            ExprKind::Tuple(items) | ExprKind::List(items) => {
                items.iter().for_each(|i| self.expr(i))
            }
            ExprKind::Map(pairs) => pairs.iter().for_each(|(k, v)| {
                self.expr(k);
                self.expr(v);
            }),
            ExprKind::Lambda { params, body } => self.lambda(params, body),
            ExprKind::Unary { expr, .. } => self.expr(expr),
            ExprKind::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Range { from, to, .. } => {
                self.expr(from);
                self.expr(to);
            }
            _ => self.compound(e),
        }
    }

    /// The expressions made of others with types, blocks and arms.
    fn compound(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Is { expr, ty, bind, .. } => {
                self.expr(expr);
                self.ty(ty);
                if let Some(n) = bind {
                    self.bind(n, DefKind::Value);
                }
            }
            ExprKind::As { expr, ty, .. } => {
                self.expr(expr);
                self.ty(ty);
            }
            ExprKind::Call { callee, args } => {
                self.expr(callee);
                args.iter().for_each(|a| self.expr(&a.value));
            }
            ExprKind::Index { expr, index } => {
                self.expr(expr);
                self.expr(index);
            }
            ExprKind::TypeArgs { expr, args } => {
                self.expr(expr);
                args.iter().for_each(|t| self.ty(t));
            }
            // A member is the object's: looked up with its type.
            ExprKind::Member { expr, .. } => self.expr(expr),
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => {
                self.scoped(|r| {
                    r.cond(cond);
                    r.block(then);
                });
                if let Some(o) = otherwise {
                    self.expr(o);
                }
            }
            ExprKind::Match { expr, arms } => {
                self.expr(expr);
                for a in arms {
                    self.scoped(|r| {
                        r.bind_pattern(&a.pattern);
                        r.opt_expr(a.guard.as_ref());
                        r.expr(&a.body);
                    });
                }
            }
            ExprKind::Block(b) => self.block(b),
            _ => {}
        }
    }

    /// A lambda: its parameters (none: `it`), its own loops.
    fn lambda(&mut self, params: &[crate::ast::Name], body: &[crate::ast::Stmt]) {
        let cx = Context {
            loops: 0,
            it: params.is_empty() || self.cx.it,
            before: false,
            member: self.cx.member,
        };
        self.within(cx, |r| {
            r.scoped(|r| {
                for p in params {
                    r.bind(p, DefKind::Param);
                }
                r.stmts(body);
            });
        });
    }

    pub(super) fn opt_ty(&mut self, t: Option<&Type>) {
        if let Some(t) = t {
            self.ty(t);
        }
    }

    /// A type: its first name looked up (the rest are inside it).
    pub(super) fn ty(&mut self, t: &Type) {
        match &t.kind {
            TypeKind::Path { path, args } => {
                if let Some(first) = path.first() {
                    self.refer(&first.text, first.span);
                }
                args.iter().for_each(|a| self.ty(a));
            }
            TypeKind::Bounded { base, range } => {
                self.ty(base);
                self.expr(range);
            }
            TypeKind::Tuple(items) => items.iter().for_each(|i| self.ty(i)),
            TypeKind::Fn { params, ret } => {
                params.iter().for_each(|p| self.ty(p));
                self.ty(ret);
            }
            TypeKind::Optional(inner) => self.ty(inner),
        }
    }
}
