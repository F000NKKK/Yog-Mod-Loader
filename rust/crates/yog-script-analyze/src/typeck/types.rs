//! Each expression's type, as the checker found it: what a compiler needs
//! to write the code (a local's declaration, a value boxed, a switch's
//! result) and an editor to show.

use std::collections::HashMap;

use super::model::Type;
use crate::ast::Expr;

/// The types of a checked file's expressions. An expression is known by
/// its node in the file's tree — which, once checked, isn't changed — so
/// two that share a place (a macro's output, all at its call) stay apart.
#[derive(Debug, Default)]
pub struct Types(HashMap<usize, Type>);

impl Types {
    /// `e`'s type, if it was checked.
    pub fn of(&self, e: &Expr) -> Option<&Type> {
        self.0.get(&key(e))
    }

    pub(super) fn set(&mut self, e: &Expr, t: Type) {
        self.0.insert(key(e), t);
    }
}

/// A node's address: its identity in the tree.
fn key(e: &Expr) -> usize {
    std::ptr::from_ref(e) as usize
}
