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
pub struct Types {
    types: HashMap<usize, Type>,
    /// The operators a type declares that expressions are (`a + b`, `-a`,
    /// `(Energy)n`), by expression.
    operators: HashMap<usize, OperatorUse>,
    /// The implicit conversions values go through where they're handed
    /// on, by expression.
    conversions: HashMap<usize, OperatorUse>,
}

/// An operator or a conversion a type declares, as used: the type, its
/// method (`op_Addition`, `op_Implicit`).
#[derive(Debug, Clone, PartialEq)]
pub struct OperatorUse {
    pub owner: Type,
    pub method: String,
}

impl Types {
    /// `e`'s type, if it was checked.
    pub fn of(&self, e: &Expr) -> Option<&Type> {
        self.types.get(&key(e))
    }

    /// The declared operator `e` is, if it's one.
    pub fn operator(&self, e: &Expr) -> Option<&OperatorUse> {
        self.operators.get(&key(e))
    }

    /// The implicit conversion `e`'s value goes through, if any.
    pub fn conversion(&self, e: &Expr) -> Option<&OperatorUse> {
        self.conversions.get(&key(e))
    }

    pub(super) fn set(&mut self, e: &Expr, t: Type) {
        self.types.insert(key(e), t);
    }

    pub(super) fn set_operator(&mut self, e: &Expr, used: OperatorUse) {
        self.operators.insert(key(e), used);
    }

    pub(super) fn set_conversion(&mut self, e: &Expr, used: OperatorUse) {
        self.conversions.insert(key(e), used);
    }
}

/// A node's address: its identity in the tree.
fn key(e: &Expr) -> usize {
    std::ptr::from_ref(e) as usize
}
