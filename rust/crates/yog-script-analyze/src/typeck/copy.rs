//! Which values are copied and which are handed on (`ys.md` §3, Values
//! and objects): a `Copy` value goes anywhere by itself; any other is
//! moved (`move a`), lent (`in a`) or cloned — a plain `b = a` of one, or
//! one passed by value without `move`, is reported.

use std::collections::HashSet;

use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
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

    fn copy_seen(&self, t: &Type, seen: &mut HashSet<String>) -> bool {
        match t {
            Type::List(_) | Type::Map(..) | Type::Object => false,
            Type::Nullable(inner) => self.copy_seen(inner, seen),
            Type::Tuple(items) => items.iter().all(|i| self.copy_seen(i, seen)),
            Type::Own(name, _) => self.own_copy(name, seen),
            // Numbers, strings, handles, delegates, what isn't known.
            _ => true,
        }
    }

    /// An own type: an object never; a struct or an enum when everything
    /// it holds is (a type holding itself is, as far as itself goes).
    fn own_copy(&self, name: &str, seen: &mut HashSet<String>) -> bool {
        let Some(own) = self.own.types.get(name) else {
            return true;
        };
        if own.heap || !seen.insert(name.to_string()) {
            return !own.heap;
        }
        let fields = own
            .values
            .values()
            .chain(own.variants.values().flatten().map(|p| &p.ty));
        fields
            .into_iter()
            .all(|f| match super::written::simple_path(f) {
                Some(n) if self.own.types.contains_key(n) => self.own_copy(n, seen),
                _ => !written_heap(f),
            })
    }
}

/// A field's written type that is a collection (a heap object of the
/// library's): its holder isn't `Copy`.
fn written_heap(t: &crate::ast::Type) -> bool {
    use super::written::{LIST, MAP, SET};
    match &t.kind {
        crate::ast::TypeKind::Path { path, .. } => path
            .last()
            .is_some_and(|n| [LIST, MAP, SET].contains(&n.text.as_str())),
        _ => false,
    }
}
