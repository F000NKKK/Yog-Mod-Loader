//! A type parameter's value as its constraints give it (C#'s `where`):
//! reached through its bound (`Generics.view(t, IScore.class).Score()`, a
//! number's interface its companion's), made (`new T()`), its type's
//! default (`default(T)`) — each by the type the code reaches `T` as.

use yog_script_analyze::Type;
use yog_script_analyze::ast::Expr;
use yog_script_ids::java::{CLASS_LITERAL, NULL};
use yog_script_ids::jvm::generics::{GENERICS, MADE, VIEW, ZERO};

use crate::Gen;

impl Gen<'_> {
    /// `target` (the value of `on`, of type `on_type`): through its bound
    /// when it's a type parameter's reached so, its type then the bound's.
    pub(crate) fn through_bound(
        &self,
        on: &Expr,
        target: String,
        on_type: Option<Type>,
    ) -> (String, Option<Type>) {
        let Some(bound @ Type::Own(name, args)) = self.types.bounded(on) else {
            return (target, on_type);
        };
        let class = format!("{}{CLASS_LITERAL}", self.index.place(name, args.len()));
        (
            format!("{GENERICS}.{VIEW}({target}, {class})"),
            Some(bound.clone()),
        )
    }

    /// `new T()` (`t` a type parameter): made of its type as reached here.
    pub(crate) fn param_made(&self, t: &Type) -> Option<String> {
        let Type::Param(name) = t else {
            return None;
        };
        let value = self.type_param(name).unwrap_or_else(|| NULL.into());
        Some(format!("(({name}) {GENERICS}.{MADE}({value}))"))
    }

    /// `default(T)` (`t` a type parameter): its type's zero as reached here.
    pub(crate) fn param_zero(&self, t: &Type) -> Option<String> {
        let Type::Param(name) = t else {
            return None;
        };
        let value = self.type_param(name)?;
        Some(format!("(({name}) {GENERICS}.{ZERO}({value}))"))
    }
}
