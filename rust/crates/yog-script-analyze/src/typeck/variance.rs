//! Variance, as C#'s: an `out` type parameter's argument may be one that
//! converts to the one wanted (`IEnumerable<string>` is an
//! `IEnumerable<object>`), an `in` one's the other way (`Action<object>` is
//! an `Action<string>`) — reference types' only. Where a value goes so, the
//! JVM's invariant generics need it recast (`Types::recast`).

use super::Checker;
use super::model::Type;
use crate::ast::{Expr, Variance};

impl Checker<'_, '_, '_> {
    /// Whether own type `a<xs>` is `b<ys>`: it, or a type it inherits, of
    /// the arguments `ys` — or, `varied`, ones its variance lets through.
    pub(super) fn own_converts(
        &self,
        a: &str,
        xs: &[Type],
        (b, ys): (&str, &[Type]),
        varied: bool,
    ) -> bool {
        self.lineage_of(a, xs).iter().any(|(k, args)| {
            if k.name != b || k.arity != ys.len() {
                return false;
            }
            let variance = self
                .own
                .of_key(k)
                .map(|o| o.variance.clone())
                .unwrap_or_default();
            args.iter().zip(ys).enumerate().all(|(i, (x, y))| {
                if open(x) || open(y) || x == y {
                    return true;
                }
                varied
                    && match variance.get(i).copied().flatten() {
                        Some(Variance::Out) => self.is_reference(x) && self.converts_plainly(x, y),
                        Some(Variance::In) => self.is_reference(y) && self.converts_plainly(y, x),
                        None => false,
                    }
            })
        })
    }

    /// `e` (a `got`) handed where `wanted` is through its variance only: the
    /// type it's recast to noted.
    pub(super) fn note_variance(&mut self, e: &Expr, got: &Type, wanted: &Type) {
        if let (Type::Own(a, xs), Type::Own(b, ys)) = (got.non_null(), wanted.non_null())
            && !self.own_converts(a, xs, (b, ys), false)
            && self.own_converts(a, xs, (b, ys), true)
        {
            self.types.set_recast(e, wanted.non_null().clone());
        }
    }
}

/// A type argument not settled yet (a case made without the others':
/// `Ok(5)` says nothing of `E`): it fits whatever is wanted.
fn open(t: &Type) -> bool {
    matches!(t, Type::Param(_) | Type::Unknown)
}
