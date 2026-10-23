//! A declaration's type parameters and what they must be (`ys-grammar.md`
//! §2.3): `<in T, out R>`, `where T : class, new(), IComparable<T>`.

use super::{Name, Type};

/// `T`, `in T`, `out T`.
#[derive(Debug, Clone, PartialEq)]
pub struct Generic {
    pub name: Name,
    pub variance: Option<Variance>,
}

/// An interface's or a delegate's type parameter: taken (`in`) or given
/// (`out`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variance {
    In,
    Out,
}

/// One thing a type parameter must be.
#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    /// `class`: an object (a reference).
    Class,
    /// `struct`: a value.
    Struct,
    /// `new()`: made with no arguments.
    New,
    /// A base class or an interface.
    Type(Type),
}

/// `where T : …`.
#[derive(Debug, Clone, PartialEq)]
pub struct Where {
    pub name: Name,
    pub bounds: Vec<Constraint>,
}
