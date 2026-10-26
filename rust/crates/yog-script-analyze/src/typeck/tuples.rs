//! Tuples, as C#'s: `(a, b)`, `(X: 1, Y: 2)` — their items read by their
//! names (given, or inferred from a name or a member written: `(p.Name,
//! age)` names `Name` and `age`) or as `Item1`, `Item2`…; one tuple type
//! goes where another of its length is wanted, item by item, names aside.

use yog_ids::script_problem;
use yog_script_ids::clr::TUPLE_ITEM;
use yog_script_ids::system::TUPLE_ITEMS;

use super::Checker;
use super::generics::Bound;
use super::model::Type;
use crate::ast::{Expr, ExprKind, Name};
use crate::source::token::Span;

impl Checker<'_, '_, '_> {
    /// `(a, b)` alone: its items' types, their names.
    pub(super) fn tuple(&mut self, items: &[Expr], names: &[Option<Name>], at: Span) -> Type {
        if items.len() > TUPLE_ITEMS {
            self.report.error(
                script_problem::TYPE_MISMATCH,
                at,
                format!("a tuple holds {TUPLE_ITEMS} items at most"),
            );
        }
        let types = items.iter().map(|i| self.expr(i)).collect();
        let names = items
            .iter()
            .zip(names)
            .map(|(i, n)| n.as_ref().map(|n| n.text.clone()).or_else(|| inferred(i)))
            .collect();
        Type::Tuple(types, names)
    }

    /// `(a, b)` (`e`) where tuple type `wanted` is: each item fitting its
    /// own. Whether `wanted` is a tuple of its length.
    pub(super) fn tuple_as(&mut self, e: &Expr, wanted: &Type) -> bool {
        let (ExprKind::Tuple { items, .. }, Type::Tuple(types, _)) = (&e.kind, wanted) else {
            return false;
        };
        if items.len() != types.len() {
            return false;
        }
        for (item, t) in items.iter().zip(types) {
            self.fits(item, t);
        }
        self.types.set(e, wanted.clone());
        true
    }

    /// Item `name` of a tuple of `types` named `names`: its type.
    pub(super) fn tuple_item(types: &[Type], names: &[Option<String>], name: &str) -> Option<Type> {
        let by_name = names.iter().position(|n| n.as_deref() == Some(name));
        let by_number = name
            .strip_prefix(TUPLE_ITEM)
            .and_then(|n| n.parse::<usize>().ok())
            .and_then(|n| n.checked_sub(1));
        by_name.or(by_number).and_then(|i| types.get(i).cloned())
    }

    /// Whether tuple `got` goes where tuple `wanted` is: as long, each item
    /// converting to its own.
    pub(super) fn tuple_converts(&self, got: &[Type], wanted: &[Type]) -> bool {
        got.len() == wanted.len()
            && got
                .iter()
                .zip(wanted)
                .all(|(g, w)| self.converts_plainly(g, w))
    }

    /// `param`'s items bound by `given`'s (generic inference).
    pub(super) fn unify_tuples(
        &self,
        (param, given): (&[Type], &[Type]),
        free: &[String],
        found: &mut Bound,
    ) -> bool {
        param.len() == given.len()
            && param
                .iter()
                .zip(given)
                .all(|(p, g)| self.unify(p, g, free, found))
    }
}

/// The name C# gives an item written as a name or a member (`a`, `p.Name`).
fn inferred(item: &Expr) -> Option<String> {
    match &item.kind {
        ExprKind::Ident(name) => Some(name.clone()),
        ExprKind::Member { name, .. } => Some(name.text.clone()),
        _ => None,
    }
}
