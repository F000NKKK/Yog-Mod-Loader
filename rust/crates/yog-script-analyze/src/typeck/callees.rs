//! What `on.Name(…)` calls: a method of `on`'s type (or what it inherits),
//! a case of an enum, Yog's or the game's method, a built-in type's
//! (`[Host]`), else an extension method in reach.

use super::Checker;
use super::members::{Callee, own_args};
use super::model::Type;
use crate::ast::{Expr, Name};

impl Checker<'_, '_, '_> {
    /// `on.name(…)`: what's called (`on` is `expr`).
    pub(super) fn method_of(&mut self, expr: &Expr, name: &Name) -> Callee {
        let on = self.expr(expr);
        if let Some(methods) = self.nullable_methods(&on, &name.text) {
            return Callee::Own(methods);
        }
        match on.non_null() {
            Type::Own(of, _) | Type::Static(of, _) => self.own_method_of(expr, &on, of, name),
            Type::Yog(group) => match self.extension_methods(group, &name.text) {
                Some(m) => Callee::Own(m),
                None => self.yog_methods(group, &name.text, name.span),
            },
            Type::Param(p) => match self.param_methods(p, &name.text) {
                Some((bound, methods)) => {
                    self.types.set_bounded(expr, bound);
                    Callee::Own(methods)
                }
                None => self.object_methods(on.non_null(), name),
            },
            Type::Game(class) => match self.extension_methods(class, &name.text) {
                Some(m) => Callee::Own(m),
                None if super::members::is_side_call(&name.text) => Callee::Unknown,
                None => self.game_methods(class, &name.text, name.span),
            },
            other => match self
                .hosted(other)
                .and_then(|host| self.own_methods(&host, &name.text))
            {
                Some(methods) => Callee::Own(methods),
                None => self.extension_or_unknown(other, name),
            },
        }
    }

    /// `on.name(…)`, `on` of own type `of` (or `of` itself, a static's).
    fn own_method_of(&mut self, expr: &Expr, on: &Type, of: &str, name: &Name) -> Callee {
        let instance = matches!(on.non_null(), Type::Own(..));
        let on_type = match on.non_null() {
            Type::Static(_, args) => self.statics_type(of, args),
            other => Type::Own(of.to_string(), own_args(other).to_vec()),
        };
        if let Some(methods) = self.own_methods(&on_type, &name.text) {
            if instance {
                self.called_on(Some(expr), of, &name.text, name.span);
            }
            return Callee::Own(methods);
        }
        match self.variant_ctor(of, &name.text) {
            Callee::Unknown if instance => self.extension_or_unknown(on.non_null(), name),
            Callee::Unknown => {
                self.unknown_member(on.non_null(), &name.text, name.span);
                Callee::Unknown
            }
            made => made,
        }
    }
}
