//! Method groups, as C#'s: a method named where a delegate goes
//! (`xs.Select(Twice)`, `Func<i32, i32> f = c.Times`, `Calc.Plus`) is one
//! of it — the overload of the delegate's shape, noted for the compiler as
//! a call's is. A group's result types a generic call it's given to
//! (`Select(Twice)` a `Select<i32, i32>`).

use yog_ids::script_problem;

use super::Checker;
use super::context::Context;
use super::model::Type;
use crate::ast::{Expr, ExprKind, Method};

impl Checker<'_, '_, '_> {
    /// The methods `e` names when it's a method group: a name no local
    /// holds, or a type's or an object's member that's a method (not a
    /// value of a delegate's type).
    fn group(&mut self, e: &Expr) -> Option<Vec<(Method, Context)>> {
        let methods = match &e.kind {
            ExprKind::Ident(name) if self.local(name).is_none() => self.callable(name),
            ExprKind::Member {
                expr,
                name,
                safe: false,
            } => {
                let on = self.quietly(|c| c.expr(expr));
                let owner = match on.non_null() {
                    Type::Static(n, args) => Type::Own(n.clone(), args.clone()),
                    t => t.clone(),
                };
                if self.quietly(|c| c.own_member(&owner, &name.text)).known() {
                    return None;
                }
                self.own_methods(&owner, &name.text).unwrap_or_default()
            }
            _ => return None,
        };
        (!methods.is_empty()).then_some(methods)
    }

    /// Whether `e` is a method group.
    pub(super) fn is_group(&mut self, e: &Expr) -> bool {
        self.quietly(|c| c.group(e)).is_some()
    }

    /// Method group `e` where a delegate taking `params` and giving `ret`
    /// is wanted: its overload of that shape chosen and noted, or none
    /// reported. Whether `e` is a group.
    pub(super) fn group_as(&mut self, e: &Expr, params: &[Type], ret: &Type) -> bool {
        let Some(methods) = self.quietly(|c| c.group(e)) else {
            return false;
        };
        if let ExprKind::Member { expr, .. } = &e.kind {
            self.expr(expr);
        }
        let wanted = Type::Fn(params.to_vec(), Box::new(ret.clone()));
        match self.of_shape(&methods, params, Some(ret)) {
            Some((m, context)) => {
                let used = self.method_use(&m, &context);
                self.types.set_method(e, used);
                self.types.set(e, wanted);
            }
            None => self.report.error(
                script_problem::TYPE_MISMATCH,
                e.span,
                format!("no overload of this method is a `{wanted}`"),
            ),
        }
        true
    }

    /// What method group `e` gives taking `params`, when it's one with an
    /// overload taking those.
    pub(super) fn group_result(&mut self, e: &Expr, params: &[Type]) -> Option<Type> {
        let methods = self.quietly(|c| c.group(e))?;
        let (m, context) = self.of_shape(&methods, params, None)?;
        Some(self.in_context(&context, |c| c.signature_types(&m)).1)
    }

    /// The first of `methods` taking `params` (and giving what `ret` takes,
    /// when it's said).
    fn of_shape(
        &mut self,
        methods: &[(Method, Context)],
        params: &[Type],
        ret: Option<&Type>,
    ) -> Option<(Method, Context)> {
        methods
            .iter()
            .find(|(m, context)| {
                let (taken, given) = self.in_context(context, |c| c.signature_types(m));
                taken.len() == params.len()
                    && params.iter().zip(&taken).all(|(p, t)| p.converts_to(t))
                    && ret.is_none_or(|r| given.converts_to(r))
            })
            .cloned()
    }
}
