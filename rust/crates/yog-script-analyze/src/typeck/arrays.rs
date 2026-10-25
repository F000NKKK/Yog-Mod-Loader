//! Arrays, as C#'s: made with their sizes or items (`new i32[n]`,
//! `new[] { 1, 2 }`, `new i32[,] { { 1, 2 }, { 3, 4 } }`), read by as many
//! whole-number indices as their rank; a reference type's array is one of
//! any type it converts to (covariance), and a `T[]` is an `IList<T>` and
//! the rest of its interfaces.

use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
use super::types::OperatorUse;
use crate::ast::{self, ArrayItem, Expr};
use crate::source::token::Span;
use yog_script_ids::system::{ARRAY, ARRAY_INTERFACES, ARRAY_VIEW};

impl Checker<'_, '_, '_> {
    /// `new T[sizes] { init }` / `new[] { init }`: an array of `rank`.
    pub(super) fn new_array(
        &mut self,
        elem: Option<&ast::Type>,
        rank: usize,
        sizes: &[Expr],
        init: Option<&[ArrayItem]>,
        at: Span,
    ) -> Type {
        for s in sizes {
            self.whole(s);
        }
        let item = match elem {
            Some(t) => self.type_of(t),
            None => self.items_own(init.unwrap_or_default()),
        };
        if let Some(items) = init {
            self.array_items(items, rank, &item, at);
        }
        Type::Array(Box::new(item), rank)
    }

    /// `new[] { … }`'s item type: its first item's.
    fn items_own(&mut self, items: &[ArrayItem]) -> Type {
        let mut first = None;
        for i in items {
            i.each_value(&mut |e| {
                first.get_or_insert(e);
            });
        }
        first.map_or(Type::Unknown, |e| self.expr(e))
    }

    /// An initializer's items, nested as deep as `rank`, each fitting `item`.
    fn array_items(&mut self, items: &[ArrayItem], rank: usize, item: &Type, at: Span) {
        for i in items {
            match (i, rank) {
                (ArrayItem::Value(v), 1) => self.fits(v, item),
                (ArrayItem::Nested(inner), r) if r > 1 => self.array_items(inner, r - 1, item, at),
                // Said once for the level: its items are all of a shape.
                _ => {
                    return self.report.error(
                        script_problem::TYPE_MISMATCH,
                        at,
                        format!(
                            "an array of rank {rank}'s initializer nests its items {rank} deep"
                        ),
                    );
                }
            }
        }
    }

    /// `a[i, j]`: as many indices as the array's rank, whole numbers.
    pub(super) fn array_index(&mut self, index: &[Expr], rank: usize, at: Span) {
        if index.len() != rank {
            self.report.error(
                script_problem::WRONG_ARGS,
                at,
                format!("an array of rank {rank} takes {rank} indices"),
            );
        }
        for i in index {
            self.whole(i);
        }
    }

    /// `e`, where a whole number is wanted (a size, an index).
    fn whole(&mut self, e: &Expr) {
        let t = self.expr(e);
        if t.known() && !t.is_integer() {
            self.report.error(
                script_problem::TYPE_MISMATCH,
                e.span,
                format!("a `{t}` where a whole number is wanted"),
            );
        }
    }

    /// Whether array `got` goes where `wanted` is: an array of its rank of
    /// a type its items convert to (a reference type's), one of a `T[]`'s
    /// interfaces, `System.Array`.
    pub(super) fn array_converts(&self, got: &Type, wanted: &Type) -> bool {
        let Type::Array(item, rank) = got else {
            return false;
        };
        let item_fits = |to: &Type| {
            **item == *to || (self.is_reference(item) && self.converts_plainly(item, to))
        };
        match wanted {
            Type::Array(to, r) => r == rank && item_fits(to),
            Type::Own(name, args) if name == ARRAY => args.is_empty(),
            Type::Own(name, args) => {
                *rank == 1
                    && ARRAY_INTERFACES.contains(&name.as_str())
                    && args.len() == 1
                    && item_fits(&args[0])
            }
            _ => false,
        }
    }

    /// `e`, an array, handed where one of its interfaces is wanted: the view
    /// it goes through noted (the JVM's arrays aren't their interfaces).
    pub(super) fn note_array_view(&mut self, e: &Expr, got: &Type, wanted: &Type) {
        if matches!(got.non_null(), Type::Array(..))
            && matches!(wanted.non_null(), Type::Own(name, _) if name != ARRAY)
        {
            let used = OperatorUse {
                owner: Type::own(ARRAY.to_string()),
                method: ARRAY_VIEW.to_string(),
            };
            self.types.set_conversion(e, used);
        }
    }

    /// Whether a `t` is a reference (an array of it is one of a base's).
    fn is_reference(&self, t: &Type) -> bool {
        match t {
            Type::String | Type::Object | Type::Dynamic | Type::Array(..) => true,
            Type::Own(name, args) => self.own.get(name, args.len()).is_some_and(|o| o.heap),
            _ => false,
        }
    }
}
