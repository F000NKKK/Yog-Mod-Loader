//! Expressions and types: names looked up, `it`, `this` and `await` where
//! they mean something, lambdas and switch arms in scopes of their own.

use yog_ids::script_problem;

use super::{Context, DefKind, Resolver};
use crate::ast::{
    Arg, Element, Expr, ExprKind, IT, LambdaBody, LambdaParam, Pass, StrSeg, Type, TypeKind, UnOp,
};

impl Resolver<'_, '_> {
    pub(super) fn opt_expr(&mut self, e: Option<&Expr>) {
        if let Some(e) = e {
            self.expr(e);
        }
    }

    pub(super) fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Ident(name) => self.name_use(name, e),
            ExprKind::This if !self.cx.member => self.report.error(
                script_problem::THIS_OUTSIDE_MEMBER,
                e.span,
                "`this` is the object in a type's, a content's, a component's or a patch's members",
            ),
            ExprKind::Base if !self.cx.member => self.report.error(
                script_problem::THIS_OUTSIDE_MEMBER,
                e.span,
                "`base` is the object as its base class, in a class's members",
            ),
            ExprKind::TypeOf(ty) => self.ty(ty),
            ExprKind::Declare { ty, name, mutable } => {
                self.opt_ty(ty.as_ref());
                self.bind(name, DefKind::local(*mutable));
            }
            ExprKind::NameOf(expr) => self.expr(expr),
            ExprKind::Default(ty) => self.opt_ty(ty.as_ref()),
            ExprKind::Str(segs) => {
                for s in segs {
                    if let StrSeg::Expr(e) = s {
                        self.expr(e);
                    }
                }
            }
            ExprKind::Tuple(items) => items.iter().for_each(|i| self.expr(i)),
            ExprKind::Collection(items) => items.iter().for_each(|i| match i {
                Element::Item(e) | Element::Spread(e) => self.expr(e),
                Element::Entry(k, v) => {
                    self.expr(k);
                    self.expr(v);
                }
            }),
            ExprKind::New { ty, args, init } => {
                self.opt_ty(ty.as_ref());
                args.iter().flatten().for_each(|a| self.expr(&a.value));
                init.iter().for_each(|(_, v)| self.expr(v));
            }
            ExprKind::Lambda { params, body } => self.lambda(params, body),
            ExprKind::Unary { op, expr } => {
                if *op == UnOp::Await && !self.cx.awaits {
                    self.report.error(
                        script_problem::AWAIT_OUTSIDE_ASYNC,
                        e.span,
                        "`await` waits in an `async` method: `async void Arm() { await Task.Delay(5s); }`",
                    );
                }
                self.expr(expr);
            }
            _ => self.compound(e),
        }
    }

    /// An argument: what `ref` or `out` lends must be assignable.
    fn arg(&mut self, a: &Arg) {
        match a.pass {
            Some(Pass::Ref | Pass::Out) => self.assigned(&a.value),
            _ => self.expr(&a.value),
        }
    }

    /// A name: `it` in a block lambda, else the source's own when it has one.
    fn name_use(&mut self, name: &str, e: &Expr) {
        if self.refer(name, e.span).is_some() || name != IT {
            return;
        }
        if !self.cx.it {
            self.report.error(
                script_problem::IT_OUTSIDE_LAMBDA,
                e.span,
                "`it` is a block lambda's argument: `list.Select { it * 2 }`",
            );
        }
    }

    /// The expressions made of others, types and arms.
    fn compound(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Cast { ty, expr }
            | ExprKind::As { expr, ty }
            | ExprKind::Has { expr, ty } => {
                self.ty(ty);
                self.expr(expr);
            }
            ExprKind::Binary {
                left: a, right: b, ..
            }
            | ExprKind::Range { from: a, to: b, .. } => {
                self.expr(a);
                self.expr(b);
            }
            // The pattern's names reach what follows (`x is Monster m && m.IsAlive`).
            ExprKind::Is { expr, pattern } => {
                self.expr(expr);
                self.bind_pattern(pattern);
            }
            ExprKind::Conditional {
                cond,
                then,
                otherwise,
            } => {
                self.expr(cond);
                self.expr(then);
                self.expr(otherwise);
            }
            ExprKind::Switch { expr, arms } => {
                self.expr(expr);
                for a in arms {
                    self.scoped(|r| {
                        r.bind_pattern(&a.pattern);
                        r.opt_expr(a.guard.as_ref());
                        r.expr(&a.value);
                    });
                }
            }
            ExprKind::Call { callee, args } => {
                self.expr(callee);
                args.iter().for_each(|a| self.arg(a));
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
            ExprKind::Member { expr, .. } | ExprKind::NotNull(expr) => self.expr(expr),
            _ => {}
        }
    }

    /// A lambda: its parameters (none: `it`), its own loops.
    fn lambda(&mut self, params: &[LambdaParam], body: &LambdaBody) {
        let cx = Context {
            loops: 0,
            it: params.is_empty() || self.cx.it,
            before: false,
            ..self.cx
        };
        self.within(cx, |r| {
            r.scoped(|r| {
                for p in params {
                    r.opt_ty(p.ty.as_ref());
                    r.bind(&p.name, DefKind::Param);
                }
                match body {
                    LambdaBody::Expr(e) => r.expr(e),
                    LambdaBody::Block(stmts) => r.stmts(stmts),
                }
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
            TypeKind::Tuple(items) => items.iter().for_each(|(t, _)| self.ty(t)),
            TypeKind::Nullable(inner) => self.ty(inner),
            TypeKind::Both(all) => all.iter().for_each(|t| self.ty(t)),
        }
    }
}
