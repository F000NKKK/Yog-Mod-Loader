//! Where a declaration's written types are read: in its own type's
//! namespace, the type's parameters bound — never from the code using it
//! (a `Vector3` member is a `System.Numerics` type wherever it's reached).

use super::Checker;
use super::generics::{Bound, params_of_names};
use super::model::Type;
use super::own::TypeKey;

/// A declaration's context: its type (none: the file's), its type's
/// parameters bound, its namespace, its file's `using`s (none: the code's).
#[derive(Debug, Clone, Default)]
pub(super) struct Context {
    pub owner: Option<TypeKey>,
    pub bound: Bound,
    pub ns: Vec<String>,
    pub usings: Option<Vec<Vec<String>>>,
}

impl Checker<'_, '_, '_> {
    /// Own type `name<args>`'s context.
    pub(super) fn context_of(&self, name: &str, args: &[Type]) -> Context {
        Context {
            owner: Some(TypeKey::new(name, args.len())),
            bound: self.bound_of(name, args),
            ns: self.ns_of(name),
            usings: self.own.usings.get(name).cloned(),
        }
    }

    /// The code's own context here, nothing bound (the file's methods, an
    /// extension's).
    pub(super) fn here(&self) -> Context {
        Context {
            owner: None,
            bound: Bound::new(),
            ns: self.ns.clone(),
            usings: None,
        }
    }

    /// The namespace own type `name` is declared in.
    pub(super) fn ns_of(&self, name: &str) -> Vec<String> {
        self.own.namespaces.get(name).cloned().unwrap_or_default()
    }

    /// Runs `f` in `context`.
    pub(super) fn in_context<R>(&mut self, context: &Context, f: impl FnOnce(&mut Self) -> R) -> R {
        let outer = std::mem::replace(&mut self.ns, context.ns.clone());
        let usings = context
            .usings
            .clone()
            .map(|u| std::mem::replace(&mut self.directives.usings, u));
        let out = self.with_types(context.bound.clone(), f);
        self.ns = outer;
        if let Some(u) = usings {
            self.directives.usings = u;
        }
        out
    }

    /// Runs `f` as in own type `name`'s declaration, its type parameters
    /// (`generics`) standing for themselves.
    pub(super) fn as_declared<R>(
        &mut self,
        name: &str,
        generics: &[String],
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let context = Context {
            owner: Some(TypeKey::new(name, generics.len())),
            bound: params_of_names(generics),
            ns: self.ns_of(name),
            usings: self.own.usings.get(name).cloned(),
        };
        self.in_context(&context, f)
    }
}
