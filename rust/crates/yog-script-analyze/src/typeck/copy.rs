//! Which values are copied and which are handed on (`ys.md` §3, Values
//! and objects): a `Copy` value (numbers, handles, records — which never
//! change) goes anywhere by itself; any other is moved (`move a`), lent
//! (`in a`) or cloned — a plain `b = a` of one, or one passed by value
//! without `move`, is reported.

use std::collections::HashSet;

use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
use super::own::TypeKey;
use crate::ast::{Expr, ExprKind};

impl Checker<'_, '_, '_> {
    /// `value` handed where its own copy is wanted (a local, an assignment,
    /// a by-value argument): a local of a type that isn't `Copy` can't be.
    pub(super) fn handed(&mut self, value: &Expr, t: &Type) {
        let ExprKind::Ident(name) = &value.kind else {
            return;
        };
        if self.local(name).is_none() || self.is_copy(t) {
            return;
        }
        self.report.error(
            script_problem::PASS,
            value.span,
            format!(
                "a `{t}` isn't copied: hand it on with `move {name}`, lend it with `in {name}`, \
                 or `{name}.Clone()` it"
            ),
        );
    }

    /// Whether a value of `t` is copied where it goes.
    pub(super) fn is_copy(&self, t: &Type) -> bool {
        self.copy_seen(t, &mut HashSet::new())
    }

    fn copy_seen(&self, t: &Type, seen: &mut HashSet<TypeKey>) -> bool {
        match t {
            Type::Array(..) | Type::Object => false,
            Type::Nullable(inner) => self.copy_seen(inner, seen),
            Type::Tuple(items, _) => items.iter().all(|i| self.copy_seen(i, seen)),
            Type::Own(name, args) => self.own_copy(&TypeKey::new(name.clone(), args.len()), seen),
            // Numbers, strings, handles, delegates, what isn't known.
            _ => true,
        }
    }

    /// An own type: an object never; a struct or an enum when everything
    /// it holds is (a type holding itself is, as far as itself goes).
    fn own_copy(&self, key: &TypeKey, seen: &mut HashSet<TypeKey>) -> bool {
        let Some(own) = self.own.of_key(key) else {
            return true;
        };
        if own.record {
            return true;
        }
        if own.heap || !seen.insert(key.clone()) {
            return !own.heap;
        }
        let fields = own
            .values
            .values()
            .chain(own.variants.values().flatten().map(|p| &p.ty));
        fields.into_iter().all(|f| match TypeKey::of_written(f) {
            Some(k) if self.own.of_key(&k).is_some() => self.own_copy(&k, seen),
            // A built-in's, a type parameter's: copied.
            _ => true,
        })
    }
}
