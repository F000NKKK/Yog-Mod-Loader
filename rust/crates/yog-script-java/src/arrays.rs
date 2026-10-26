//! Arrays as Java's: `T[]` is Java's array of `T` (`i32[]` an `int[]`), a
//! rectangular `T[,]` Java's array of arrays (`int[][]`), read with as many
//! `[i]`; made with Java's own `new` (`new int[2][3]`, `new int[] { 1, 2 }`).
//! An array's members are `System.Array`'s, on its companion, given its
//! rank (`Array.Length(a, 2)`: Java can't tell `T[,]` from `T[][]`).

use yog_script_analyze::Type;
use yog_script_analyze::ast::{ArrayItem, Element, Expr, Type as Written};
use yog_script_ids::jvm::TYPED_ARRAY as TYPED;
use yog_script_ids::system::ARRAY;

use super::Gen;
use yog_script_ids::java::RANK;
use yog_script_ids::java::{CLASS_LITERAL, FOR, NEW, VAR};

impl Gen<'_> {
    /// `on[i, j]` of an array: `on[i][j]`.
    pub(crate) fn array_slot(on: &str, at: &[String]) -> String {
        let at: String = at.iter().map(|i| format!("[{i}]")).collect();
        format!("{on}{at}")
    }

    /// `new T[sizes]…`, `new T[] { … }`, `new[] { … }`: `e`'s type
    /// (`Type::Array`) made as Java's.
    pub(crate) fn new_array(
        &mut self,
        e: &Expr,
        sizes: &[Expr],
        init: Option<&[ArrayItem]>,
    ) -> String {
        let Some(t @ Type::Array(..)) = self.types.of(e).cloned() else {
            self.not_yet("an array of a type not known", e.span);
            return String::new();
        };
        let typed = self.java(&t, false);
        // Java makes no array of a generic type with its arguments: a raw one.
        let java = crate::reified::erased(&typed);
        match init {
            // Java takes no sizes with the items.
            Some(items) => {
                let items = self.array_items(items);
                crate::types::made_array(&typed, &items)
            }
            None => {
                let (item, ranks) = split_ranks(&java);
                let sizes: Vec<String> = sizes.iter().map(|s| self.expr(s)).collect();
                let rest = ranks.saturating_sub(sizes.len());
                format!(
                    "{NEW} {item}{}{}",
                    Self::array_slot("", &sizes),
                    RANK.repeat(rest)
                )
            }
        }
    }

    /// `{ a, b }`, `{ { 1, 2 }, { 3, 4 } }`.
    fn array_items(&mut self, items: &[ArrayItem]) -> String {
        let parts: Vec<String> = items
            .iter()
            .map(|i| match i {
                ArrayItem::Value(v) => self.handed_expr(v),
                ArrayItem::Nested(inner) => self.array_items(inner),
            })
            .collect();
        format!("{{ {} }}", parts.join(", "))
    }

    /// `[a, ..rest]` where an array is wanted (`item[]`): Java's array of
    /// the items, a spread's gone through.
    pub(crate) fn array_collection(&mut self, e: &Expr, items: &[Element]) -> String {
        let Some(t @ Type::Array(..)) = self.types.of(e).cloned() else {
            return String::new();
        };
        let java = self.java(&t, false);
        if items.iter().all(|i| matches!(i, Element::Item(_))) {
            // The items themselves (their types are known by their nodes, not copies').
            let parts: Vec<String> = items
                .iter()
                .filter_map(|i| match i {
                    Element::Item(x) => Some(self.handed_expr(x)),
                    _ => None,
                })
                .collect();
            return format!("{NEW} {java} {{ {} }}", parts.join(", "));
        }
        // A spread's length isn't known: the items gathered, then copied.
        let list = self.collection_of_list(e, items);
        format!(
            "{}.{TYPED}({list}.toArray(), {java}{CLASS_LITERAL})",
            self.array_host()
        )
    }

    /// Where `System.Array`'s companion is.
    pub(crate) fn array_host(&self) -> String {
        self.index.place(ARRAY, 0)
    }

    /// `foreach` over an array of rank 2 and more: every item, row by row,
    /// as C#'s (Java's `for` would give the rows).
    pub(crate) fn array_loops(&mut self, items: &str, rank: usize) -> (String, usize) {
        let mut over = items.to_string();
        for _ in 1..rank {
            let row = self.temp();
            self.code.open(format!("{FOR} ({VAR} {row} : {over})"));
            over = row;
        }
        (over, rank - 1)
    }

    /// A Java array type written for type `t` as written (`i32[,]`).
    pub(crate) fn written_array(&self, item: &Written, rank: usize) -> String {
        format!("{}{}", self.written(item, false), RANK.repeat(rank))
    }
}

/// `int[][]` → (`int`, 2).
fn split_ranks(java: &str) -> (&str, usize) {
    let mut item = java;
    let mut ranks = 0;
    while let Some(inner) = item.strip_suffix(RANK) {
        item = inner;
        ranks += 1;
    }
    (item, ranks)
}

/// The Java ranks of a type of `rank` (`T[,]` is `T[][]`).
pub(crate) fn ranks(rank: usize) -> String {
    RANK.repeat(rank)
}
