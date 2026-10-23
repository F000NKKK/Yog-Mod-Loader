//! Patches (`.yns`, `ys-grammar.md` §6): what reaches into others' classes.

use super::{Block, Decl, Expr, Name, Param, Type};
use crate::source::token::Span;

/// `patch LivingEntity { … }`.
#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    pub target: Type,
    pub items: Vec<PatchItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatchItem {
    /// `add saved var shield: Float = 0`, `add fn …`.
    Add(Box<Decl>),
    /// `add is Chargeable`.
    AddIs(Type),
    Before {
        method: Method,
        uses: Vec<Local>,
        body: Block,
    },
    After {
        method: Method,
        uses: Vec<Local>,
        body: Block,
    },
    Inside {
        method: Method,
        points: Vec<Point>,
    },
    /// `read foodData { … }`.
    Read {
        field: Name,
        body: Block,
    },
    /// `write experienceLevel(value) { … }`.
    Write {
        field: Name,
        value: Name,
        body: Block,
    },
    Replace {
        method: Method,
        body: Block,
    },
    Priority(i64, Span),
}

/// A target method by its signature.
#[derive(Debug, Clone, PartialEq)]
pub struct Method {
    pub name: Name,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
}

/// `var local spent: Long #1`.
#[derive(Debug, Clone, PartialEq)]
pub struct Local {
    pub write: bool,
    pub name: Name,
    pub ty: Type,
    pub ordinal: Option<i64>,
}

/// A point inside a method.
#[derive(Debug, Clone, PartialEq)]
pub enum Point {
    Around {
        callee: Callee,
        select: Option<Selector>,
        body: Block,
    },
    /// `at call Vec3.add (x) { … }`: an argument by its name.
    Arg {
        callee: Callee,
        select: Option<Selector>,
        arg: Name,
        body: Block,
    },
    BeforeCall {
        callee: Callee,
        select: Option<Selector>,
        uses: Vec<Local>,
        body: Block,
    },
    AfterCall {
        callee: Callee,
        select: Option<Selector>,
        uses: Vec<Local>,
        body: Block,
    },
    Field {
        write: bool,
        field: Name,
        select: Option<Selector>,
        uses: Vec<Local>,
        body: Block,
    },
    New {
        ty: Type,
        select: Option<Selector>,
        uses: Vec<Local>,
        body: Block,
    },
    Constant {
        value: Expr,
        to: Expr,
        select: Option<Selector>,
    },
    Local {
        local: Local,
        value: Expr,
    },
}

/// A called method: `aiStep`, `Vec3.add`, with argument types to tell
/// overloads apart (`Vec3.add(Double, Double, Double)`).
#[derive(Debug, Clone, PartialEq)]
pub struct Callee {
    pub path: Vec<Name>,
    pub args: Option<Vec<Type>>,
}

/// Which of several matching points.
#[derive(Debug, Clone, PartialEq)]
pub enum Selector {
    First,
    Last,
    Where(Expr),
}
