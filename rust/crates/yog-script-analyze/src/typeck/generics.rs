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
            .get(name, args.len())
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
            // `given` as the type `param` names, among what it inherits:
            // a `List<i32>` where an `IEnumerable<T>` is wanted binds `T`.
            (Type::Own(b, ys), Type::Own(a, xs)) => {
                let as_wanted = self
                    .lineage_of(a, xs)
                    .into_iter()
                    .find(|(k, _)| k.name == *b && k.arity == ys.len());
                match as_wanted {
                    Some((_, xs)) => ys
                        .iter()
                        .zip(&xs)
                        .all(|(y, x)| self.unify(y, x, free, found)),
                    None => false,
                }
            }
            (Type::Array(x, rx), Type::Array(y, ry)) => rx == ry && self.unify(x, y, free, found),
            // An `ItemId` where a `ContentId<T>` is wanted: `T` is `ItemId`.
            (Type::Own(b, ys), Type::Content(_)) if b == yog_script_ids::system::CONTENT_ID && ys.len() == 1 => {
                self.unify(&ys[0], given, free, found)
            }
            (Type::Nullable(x), Type::Nullable(y)) => self.unify(x, y, free, found),
            (Type::Tuple(xs, _), Type::Tuple(ys, _)) => self.unify_tuples((xs, ys), free, found),
            // A `T[]` as one of its interfaces: `IEnumerable<T>`'s `T` its item.
            (Type::Own(b, ys), Type::Array(x, 1))
                if yog_script_ids::system::ARRAY_INTERFACES.contains(&b.as_str())
                    && ys.len() == 1 =>
            {
                self.unify(&ys[0], x, free, found)
            }
            // `null` where a nullable is wanted: nothing to bind.
            (Type::Nullable(_), Type::Null) => true,
            (Type::Nullable(x), y) => self.unify(x, y, free, found),
            _ => self.converts(given, param),
        }
    }
}

/// Type parameters `names` standing for themselves.
pub(super) fn params_of_names(names: &[String]) -> Bound {
    names
        .iter()
        .map(|n| (n.clone(), Type::Param(n.clone())))
        .collect()
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
            Type::Array(t, rank) => Type::Array(Box::new(sub(t)), *rank),
            Type::Nullable(t) => Type::Nullable(Box::new(sub(t))),
            Type::Tuple(items, names) => {
                Type::Tuple(items.iter().map(sub).collect(), names.clone())
            }
            Type::Fn(params, ret) => Type::Fn(params.iter().map(sub).collect(), Box::new(sub(ret))),
            _ => self.clone(),
        }
    }
}
