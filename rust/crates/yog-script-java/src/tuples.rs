//! Tuples as Java's: a `(A, B)` is the library's `ValueTuple2<A, B>` (a
//! struct: copied where it's handed on), made with `new`, its items its
//! fields `Item1`, `Item2`… whatever names the code gave them; taken apart
//! (`let (a, b) = t;`) through a temporary.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Expr, Name};
use yog_script_ids::clr::TUPLE_ITEM;
use yog_script_ids::system::VALUE_TUPLE;

use super::Gen;
use yog_script_ids::java::{FINAL, NEW, VAR};

impl Gen<'_> {
    /// Where a tuple of `count` items is in Java.
    pub(crate) fn tuple_place(&self, count: usize) -> String {
        self.index.place(VALUE_TUPLE, count)
    }

    /// `(a, b)`.
    pub(crate) fn tuple(&mut self, items: &[Expr]) -> String {
        let items: Vec<String> = items.iter().map(|i| self.handed_expr(i)).collect();
        format!(
            "{NEW} {}<>({})",
            self.tuple_place(items.len()),
            items.join(", ")
        )
    }

    /// Item `name` of a tuple whose items are named `names`: its field.
    pub(crate) fn tuple_field(names: &[Option<String>], name: &str) -> String {
        match names.iter().position(|n| n.as_deref() == Some(name)) {
            Some(i) => format!("{TUPLE_ITEM}{}", i + 1),
            None => name.to_string(),
        }
    }

    /// `let (a, b) = v;` (`fin`: `final ` or nothing).
    pub(crate) fn taken_apart(&mut self, names: &[Name], v: &Expr, fin: &str) {
        let (whole, t) = (self.handed_expr(v), self.temp());
        self.code.line(format!("{FINAL} {VAR} {t} = {whole};"));
        for (i, n) in names.iter().enumerate() {
            let name = Self::ident(&n.text);
            self.code
                .line(format!("{fin}{VAR} {name} = {t}.{TUPLE_ITEM}{};", i + 1));
        }
    }

    /// Whether `t` is a tuple: a struct, copied where it's handed on.
    pub(crate) fn is_tuple(t: Option<&Type>) -> bool {
        matches!(t, Some(Type::Tuple(..)))
    }
}
