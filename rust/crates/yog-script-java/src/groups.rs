//! Method groups where a delegate goes: a name as a lambda calling it as a
//! call by that name would (`($t1) -> Twice($t1)`), a member as Java's
//! reference of its object — read once, where it's made, as C#'s — or of
//! its type (`c::Times`, `Calc::Plus`).

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Expr, ExprKind};
use yog_script_ids::java::METHOD_REF;

use super::Gen;

impl Gen<'_> {
    /// `e` as the delegate it is when it's a method group.
    pub(crate) fn group(&mut self, e: &Expr) -> Option<String> {
        if !matches!(e.kind, ExprKind::Ident(_) | ExprKind::Member { .. }) {
            return None;
        }
        let used = self.types.method(e)?.clone();
        let Some(Type::Fn(params, _)) = self.types.of(e) else {
            return None;
        };
        let taken = params.len();
        if self.takes_types(&used) {
            self.not_yet("a generic method as a delegate", e.span);
            return Some(String::new());
        }
        if used.owner.as_deref().is_some_and(|o| self.index.is_host(o)) {
            self.not_yet("a method group of a built-in type", e.span);
            return Some(String::new());
        }
        let n = self.called_name(e, &used.name);
        Some(match &e.kind {
            ExprKind::Member { expr, .. } => {
                let on = match self.types.of(expr) {
                    Some(Type::Static(of, args)) => self.index.place(of, args.len()),
                    _ => self.expr(expr),
                };
                format!("{on}{METHOD_REF}{n}")
            }
            _ => {
                let names: Vec<String> = (0..taken).map(|_| self.temp()).collect();
                let names = names.join(", ");
                format!("({names}) -> {n}({names})")
            }
        })
    }
}
