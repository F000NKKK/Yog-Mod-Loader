//! What a type inherits, with its type arguments: `Dictionary<K, V>`
//! implements `ICollection<KeyValuePair<K, V>>`, so `d.Add(…)` declared
//! there takes `KeyValuePair<K, V>` of `d`'s own `K` and `V`.
//!
//! Each type's bases are typed once, its type parameters standing for
//! themselves; a lineage substitutes the arguments down the chain.

use std::collections::HashMap;

use super::Checker;
use super::generics::Bound;
use super::model::Type;
use super::own::TypeKey;

/// Each type's bases, typed (its parameters as `Param`), by key.
pub(super) type Bases = HashMap<TypeKey, Vec<Type>>;

impl Checker<'_, '_, '_> {
    /// Every own type's bases typed, quietly: a base that names nothing was
    /// reported where names are resolved.
    pub(super) fn type_bases(&mut self) {
        let written: Vec<(TypeKey, Vec<String>, Vec<crate::ast::Type>)> = self
            .own
            .all()
            .map(|(k, t)| (k.clone(), t.generics.clone(), t.bases.clone()))
            .collect();
        let mut bases = Bases::new();
        self.quietly(|c| {
            for (key, generics, written) in written {
                let typed = c.as_declared(&key.name, &generics, |c| {
                    written.iter().map(|b| c.type_of(b)).collect()
                });
                bases.insert(key, typed);
            }
        });
        self.bases = bases;
    }

    /// `name<args>` and every type it inherits or implements, nearest
    /// first, each once — with the arguments `name<args>` gives it.
    pub(super) fn lineage_of(&self, name: &str, args: &[Type]) -> Vec<(TypeKey, Vec<Type>)> {
        let mut out = vec![(TypeKey::new(name, args.len()), args.to_vec())];
        let mut i = 0;
        while let Some((key, args)) = out.get(i).cloned() {
            let bound: Bound = self.bound_of(&key.name, &args);
            for base in self.bases.get(&key).into_iter().flatten() {
                if let Type::Own(n, a) = base.substitute(&bound) {
                    let k = TypeKey::new(n, a.len());
                    if !out.iter().any(|(seen, _)| *seen == k) {
                        out.push((k, a));
                    }
                }
            }
            i += 1;
        }
        out
    }

    /// `f` run with what it reports dropped.
    pub(super) fn quietly<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let kept = std::mem::take(&mut self.report.out);
        let out = f(self);
        self.report.out = kept;
        out
    }
}
