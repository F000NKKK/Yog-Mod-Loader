//! Delegates, as C#'s: a type of methods (`delegate bool Predicate<in T>(T
//! obj);`). A lambda or a method of its shape is one; calling one is
//! calling its `Invoke`. Each delegate's signature is typed once (its type
//! parameters standing for themselves), its arguments substituted where
//! it's used.

use std::collections::HashMap;

use super::Checker;
use super::model::Type;
use super::own::TypeKey;

/// A delegate's signature: what it takes, what it gives.
pub(super) type Sig = (Vec<Type>, Type);

/// Each declared delegate's signature, by key.
pub(super) type Delegates = HashMap<TypeKey, Sig>;

impl Checker<'_, '_, '_> {
    /// Every declared delegate's signature typed, quietly (what it names
    /// is the resolver's to report).
    pub(super) fn type_delegates(&mut self) {
        let declared: Vec<(TypeKey, Vec<String>, crate::ast::Method)> = self
            .own
            .all()
            .filter_map(|(k, t)| Some((k.clone(), t.generics.clone(), t.delegate.clone()?)))
            .collect();
        let mut delegates = Delegates::new();
        self.quietly(|c| {
            for (key, generics, m) in declared {
                let sig = c.as_declared(&key.name, &generics, |c| c.signature_types(&m));
                delegates.insert(key, sig);
            }
        });
        self.delegates = delegates;
    }

    /// `t`'s signature when it's a delegate: a declared one's (its type
    /// arguments bound), or a lambda's or a method's own.
    pub(super) fn delegate_sig(&self, t: &Type) -> Option<Sig> {
        match t.non_null() {
            Type::Fn(params, ret) => Some((params.clone(), (**ret).clone())),
            Type::Own(name, args) => {
                let (params, ret) = self
                    .delegates
                    .get(&TypeKey::new(name.clone(), args.len()))?;
                let bound = self.bound_of(name, args);
                Some((
                    params.iter().map(|p| p.substitute(&bound)).collect(),
                    ret.substitute(&bound),
                ))
            }
            _ => None,
        }
    }
}
