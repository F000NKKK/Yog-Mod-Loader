//! An indexer as Java writes it: its accessors are methods named as the
//! CLR names them (`get_Item`, `set_Item`, `ast::accessor_name`), so `x[i]`
//! is a call of one and `x[i] = v` of the other.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Decl, Expr, Indexer, accessor_name};
use yog_script_analyze::words::{get, set};

use super::Gen;
use crate::decls::Place;

impl Gen<'_> {
    /// An indexer declared: its accessors as methods.
    pub(crate) fn indexer(&mut self, d: &Decl, ix: &Indexer, place: Place) {
        for m in ix.getter().into_iter().chain(ix.setter()) {
            self.method(d, &m, place);
        }
    }

    /// Whether `on` is indexed by its type's own indexer (not a list's or a
    /// map's built-in indexing).
    pub(crate) fn has_indexer(&self, on: &Expr) -> bool {
        matches!(self.types.of(on).map(Type::non_null), Some(Type::Own(..)))
    }

    /// `on[at]`, `on` already written: its getter called.
    pub(crate) fn index_read(on: &str, at: &str) -> String {
        format!("{on}.{}({at})", accessor_name(get::WORD.text))
    }

    /// `on[at] = v`, `on` already written: its setter called.
    pub(crate) fn index_write(on: &str, at: &str, v: &str) -> String {
        format!("{on}.{}({at}, {v});", accessor_name(set::WORD.text))
    }
}
