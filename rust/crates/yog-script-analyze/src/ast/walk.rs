//! Walking the tree: an expression's direct sub-expressions, a
//! declaration's members — for passes that look at every one.

use super::{Decl, DeclKind, Element, Expr, ExprKind, Name, Pattern, Stmt, StrSeg};

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
            | ExprKind::Checked { expr: e, .. }
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
            Pattern::Variant { args: all, .. } | Pattern::Tuple(all) | Pattern::List(all, _) => {
                all.iter().for_each(|p| p.each_binding(f));
            }
            Pattern::Slice(Some(p)) => p.each_binding(f),
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

impl Stmt {
    /// Each expression it holds, its inner statements' too (a local
    /// function's code aside: its own).
    pub fn each_expr(&self, f: &mut dyn FnMut(&Expr)) {
        match self {
            // Its own code, apart: what it uses isn't the block's statements'.
            Stmt::Function(_) => {}
            Stmt::Switch { expr, sections } => {
                f(expr);
                for s in sections {
                    for (_, guard) in s.labels.iter().flatten() {
                        guard.iter().for_each(&mut *f);
                    }
                    s.body.iter().for_each(|x| x.each_expr(f));
                }
            }
            Stmt::Local { value, .. } => value.iter().for_each(&mut *f),
            Stmt::Assign { place, value, .. } => {
                f(place);
                f(value);
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                f(cond);
                then.each_expr(f);
                if let Some(o) = otherwise {
                    o.each_expr(f);
                }
            }
            Stmt::Foreach { iter, body, .. } => {
                f(iter);
                body.each_expr(f);
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
            } => {
                if let Some(i) = init {
                    i.each_expr(f);
                }
                cond.iter().for_each(&mut *f);
                if let Some(st) = step {
                    st.each_expr(f);
                }
                body.each_expr(f);
            }
            Stmt::While { cond, body } | Stmt::DoWhile { body, cond } => {
                f(cond);
                body.each_expr(f);
            }
            Stmt::Block(b) | Stmt::Unsafe(b, _) | Stmt::Checked(b, _) => b.stmts.iter().for_each(|s| s.each_expr(f)),
            Stmt::Return(v, _) | Stmt::Skip(v, _) | Stmt::Yield(v, _) => v.iter().for_each(&mut *f),
            Stmt::Lock { target, body, .. } => {
                f(target);
                body.each_expr(f);
            }
            Stmt::Expr(e) | Stmt::Value(e) => f(e),
            Stmt::Goto(crate::ast::Goto::Case(e), _) => f(e),
            Stmt::Break(_) | Stmt::Continue(_) | Stmt::Label(_) | Stmt::Goto(..) => {}
        }
    }
}

impl Stmt {
    /// The statements directly inside it (a local function's aside: its own code).
    pub fn inner(&self) -> Vec<&Stmt> {
        match self {
            Stmt::Block(b) | Stmt::Unsafe(b, _) | Stmt::Checked(b, _) => b.stmts.iter().collect(),
            Stmt::If {
                then, otherwise, ..
            } => std::iter::once(&**then).chain(otherwise.as_deref()).collect(),
            Stmt::Foreach { body, .. }
            | Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::Lock { body, .. } => vec![&**body],
            Stmt::For {
                init, step, body, ..
            } => init.as_deref().into_iter().chain(step.as_deref()).chain([&**body]).collect(),
            Stmt::Switch { sections, .. } => sections.iter().flat_map(|s| &s.body).collect(),
            _ => Vec::new(),
        }
    }

    /// Whether it or a statement inside it is `found` (looking into a
    /// statement only when `enter` lets it).
    pub fn any_inside(&self, found: &dyn Fn(&Stmt) -> bool, enter: &dyn Fn(&Stmt) -> bool) -> bool {
        found(self) || (enter(self) && self.inner().iter().any(|s| s.any_inside(found, enter)))
    }

    /// Whether a label is in it: a `goto` may jump there.
    pub fn has_labels(&self) -> bool {
        self.any_inside(&|s| matches!(s, Stmt::Label(_)), &|_| true)
    }

    /// Whether a `goto case` or `goto default` of the `switch` it's in is
    /// in it (not one of a `switch` inside it).
    pub fn goes_to_case(&self) -> bool {
        self.any_inside(
            &|s| matches!(s, Stmt::Goto(super::Goto::Case(_) | super::Goto::Default, _)),
            &|s| !matches!(s, Stmt::Switch { .. }),
        )
    }
}
