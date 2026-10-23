//! Generics: type parameters in scope (`T` in `class Box<T>`'s code, a
//! method's `<T>`), a generic's members read with its arguments
//! (`Box<i32>`'s `Value` is an `i32`), and a generic method's arguments
//! inferred from a call (`Max(1, 2)` is `Max<i32>`).

use std::collections::HashMap;

use super::Checker;
use super::model::Type;
use crate::ast::Generic;

/// Type parameters and what they stand for here.
pub(super) type Bound = HashMap<String, Type>;

impl Checker<'_, '_, '_> {
    /// Runs `f` with `bound`'s type parameters in scope.
    pub(super) fn with_types<R>(&mut self, bound: Bound, f: impl FnOnce(&mut Self) -> R) -> R {
        self.type_params.push(bound);
        let out = f(self);
        self.type_params.pop();
        out
    }

    /// What type parameter `name` stands for here, if one is in scope.
    pub(super) fn type_param(&self, name: &str) -> Option<Type> {
        self.type_params
            .iter()
            .rev()
            .find_map(|b| b.get(name))
            .cloned()
    }

    /// Own type `name`'s type parameters bound to `args` (unknown ones as
    /// themselves).
    pub(super) fn bound_of(&self, name: &str, args: &[Type]) -> Bound {
        let generics = self
            .own
            .types
            .get(name)
            .map(|t| t.generics.as_slice())
            .unwrap_or_default();
        generics
            .iter()
            .enumerate()
            .map(|(i, g)| {
                (
                    g.clone(),
                    args.get(i)
                        .cloned()
                        .unwrap_or_else(|| Type::Param(g.clone())),
                )
            })
            .collect()
    }

    /// `f` inferring `param`'s type parameters among `free` from `given`
    /// into `found`: whether `given` fits.
    pub(super) fn unify(
        &self,
        param: &Type,
        given: &Type,
        free: &[String],
        found: &mut Bound,
    ) -> bool {
        match (param, given) {
            (Type::Param(p), _) if free.contains(p) => match found.get(p) {
                Some(t) => {
                    let t = t.clone();
                    if self.converts(given, &t) {
                        true
                    } else if self.converts(&t, given) {
                        found.insert(p.clone(), given.clone());
                        true
                    } else {
                        false
                    }
                }
                None => {
                    found.insert(p.clone(), given.clone());
                    true
                }
            },
            (Type::Own(a, xs), Type::Own(b, ys)) if a == b && xs.len() == ys.len() => xs
                .iter()
                .zip(ys)
                .all(|(x, y)| self.unify(x, y, free, found)),
            (Type::List(x), Type::List(y)) | (Type::Nullable(x), Type::Nullable(y)) => {
                self.unify(x, y, free, found)
            }
            (Type::Nullable(x), y) => self.unify(x, y, free, found),
            (Type::Map(k, v), Type::Map(k2, v2)) => {
                self.unify(k, k2, free, found) && self.unify(v, v2, free, found)
            }
            _ => self.converts(given, param),
        }
    }
}

/// A method's or a type's own parameters, standing for themselves.
pub(super) fn params_of(generics: &[Generic]) -> Bound {
    generics
        .iter()
        .filter(|g| !g.lifetime)
        .map(|g| (g.name.text.clone(), Type::Param(g.name.text.clone())))
        .collect()
}

impl Type {
    /// The type with the parameters `bound` names replaced.
    pub(super) fn substitute(&self, bound: &Bound) -> Type {
        let sub = |t: &Type| t.substitute(bound);
        match self {
            Type::Param(p) => bound.get(p).cloned().unwrap_or_else(|| self.clone()),
            Type::Own(n, args) => Type::Own(n.clone(), args.iter().map(sub).collect()),
            Type::List(t) => Type::List(Box::new(sub(t))),
            Type::Nullable(t) => Type::Nullable(Box::new(sub(t))),
            Type::Map(k, v) => Type::Map(Box::new(sub(k)), Box::new(sub(v))),
            Type::Tuple(items) => Type::Tuple(items.iter().map(sub).collect()),
            Type::Fn(params, ret) => Type::Fn(params.iter().map(sub).collect(), Box::new(sub(ret))),
            _ => self.clone(),
        }
    }
}
