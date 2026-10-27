//! Walking the tree: an expression's direct sub-expressions, a
//! declaration's members — for passes that look at every one.

use super::{Decl, DeclKind, Element, Expr, ExprKind, Name, Pattern, StrSeg};

impl Expr {
    /// Each expression directly inside this one, in reading order; a
    /// lambda's body is not (its code runs later, on its own).
    pub fn each_child<'e>(&'e self, mut f: impl FnMut(&'e Expr)) {
        match &self.kind {
            ExprKind::Str(segs) => segs.iter().for_each(|s| {
                if let StrSeg::Expr(e) = s {
                    f(e);
                }
            }),
            ExprKind::Tuple { items, .. } | ExprKind::Macro { args: items, .. } => {
                items.iter().for_each(f)
            }
            ExprKind::Collection(items) => items.iter().for_each(|i| match i {
                Element::Item(e) | Element::Spread(e) => f(e),
                Element::Entry(k, v) => {
                    f(k);
                    f(v);
                }
            }),
            ExprKind::New { args, init, .. } => {
                args.iter().flatten().for_each(|a| f(&a.value));
                init.iter().for_each(|(_, v)| f(v));
            }
            ExprKind::Call { callee, args } => {
                f(callee);
                args.iter().for_each(|a| f(&a.value));
            }
            ExprKind::Binary { left, right, .. } => {
                f(left);
                f(right);
            }
            ExprKind::Range { from, to, .. } => {
                f(from);
                f(to);
            }
            ExprKind::Index { expr, args, .. } => {
                f(expr);
                args.iter().for_each(f);
            }
            ExprKind::NewArray { sizes, init, .. } => {
                sizes.iter().for_each(&mut f);
                init.iter().flatten().for_each(|i| i.each_value(&mut f));
            }
            ExprKind::Conditional {
                cond,
                then,
                otherwise,
            } => {
                f(cond);
                f(then);
                f(otherwise);
            }
            ExprKind::Switch { expr, arms } => {
                f(expr);
                for arm in arms {
                    arm.guard.iter().for_each(&mut f);
                    f(&arm.value);
                }
            }
            ExprKind::With { expr, inits } => {
                f(expr);
                inits.iter().for_each(|(_, v)| f(v));
            }
            _ => self.each_wrapped(f),
        }
    }

    /// The one expression a wrapping form holds (`-x`, `x.Y`, `move x`, …).
    fn each_wrapped<'e>(&'e self, mut f: impl FnMut(&'e Expr)) {
        match &self.kind {
            ExprKind::NameOf(e)
            | ExprKind::NotNull(e)
            | ExprKind::Move(e)
            | ExprKind::Unary { expr: e, .. }
            | ExprKind::Cast { expr: e, .. }
            | ExprKind::Is { expr: e, .. }
            | ExprKind::As { expr: e, .. }
            | ExprKind::Has { expr: e, .. }
            | ExprKind::TypeArgs { expr: e, .. }
            | ExprKind::Member { expr: e, .. } => f(e),
            _ => {}
        }
    }
}

impl DeclKind {
    /// The declarations inside this one: a type's, a mod's, a namespace's
    /// members.
    pub fn members(&self) -> &[Decl] {
        match self {
            DeclKind::Namespace {
                body: Some(body), ..
            }
            | DeclKind::Mod { body, .. }
            | DeclKind::Type { body, .. }
            | DeclKind::Enum { body, .. }
            | DeclKind::Content { body, .. }
            | DeclKind::Component { body, .. }
            | DeclKind::Extend { body, .. }
            | DeclKind::Screen { body, .. }
            | DeclKind::Command { body, .. } => body,
            _ => &[],
        }
    }
}

impl Pattern {
    /// The names the pattern binds (`let e`, `Monster m`), in order.
    pub fn each_binding<'p>(&'p self, f: &mut impl FnMut(&'p Name)) {
        match self {
            Pattern::Let(n) | Pattern::Type { bind: Some(n), .. } => f(n),
            Pattern::Variant { args: all, .. } | Pattern::Tuple(all) => {
                all.iter().for_each(|p| p.each_binding(f));
            }
            Pattern::Property { fields, .. } => fields.iter().for_each(|(_, p)| p.each_binding(f)),
            Pattern::Or(a, b) | Pattern::And(a, b) => {
                a.each_binding(f);
                b.each_binding(f);
            }
            Pattern::Not(p) => p.each_binding(f),
            _ => {}
        }
    }
}
