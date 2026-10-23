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
    /// `add saved f32 Shield = 0;`, `add void Zap() { … }`.
    Add(Box<Decl>),
    /// `add : IChargeable;`.
    AddBases(Vec<Type>),
    Before {
        method: Target,
        uses: Vec<Local>,
        body: Block,
    },
    After {
        method: Target,
        uses: Vec<Local>,
        body: Block,
    },
    Inside {
        method: Target,
        points: Vec<Point>,
    },
    /// `read FoodData { … }`.
    Read {
        field: Name,
        body: Block,
    },
    /// `write ExperienceLevel { … }`: `value` the new one.
    Write {
        field: Name,
        body: Block,
    },
    Replace {
        method: Target,
        body: Block,
    },
    Priority(i64, Span),
}

/// A target method by its signature: `bool Hurt(DamageSource source, f32 amount)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub ret: Option<Type>,
    pub name: Name,
    pub params: Vec<Param>,
}

/// `mut local i64 spent #1`.
#[derive(Debug, Clone, PartialEq)]
pub struct Local {
    pub write: bool,
    pub ty: Type,
    pub name: Name,
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
    /// `at call Vec3.Add (x) { … }`: an argument by its name.
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

/// A called method: `AiStep`, `Vec3.Add`, with argument types to tell
/// overloads apart (`Vec3.Add(f64, f64, f64)`).
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
