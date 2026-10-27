//! Blocks and statements (`ys-grammar.md` §3.1).

use super::{Expr, Name, Type};
use crate::source::token::Span;

/// Where a `goto` goes.
#[derive(Debug, Clone, PartialEq)]
pub enum Goto {
    /// A label of its block or one around it.
    Label(Name),
    /// The `switch` section around it whose label takes this value.
    Case(Expr),
    /// The `switch` section around it that's `default`.
    Default,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

/// What a local declaration or a `foreach` binds: one name or a
/// deconstruction (`let (a, b)`).
#[derive(Debug, Clone, PartialEq)]
pub enum Binding {
    One(Name),
    Tuple(Vec<Name>),
}

impl Binding {
    /// The names it binds.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        let names: Vec<&str> = match self {
            Binding::One(n) => vec![n.text.as_str()],
            Binding::Tuple(ns) => ns.iter().map(|n| n.text.as_str()).collect(),
        };
        names.into_iter()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `let x = …;`, `let mut x = …;`, `i32 x;`,
    /// `let (a, b) = …;`; `ty` None: its value's.
    Local {
        ty: Option<Type>,
        binding: Binding,
        value: Option<Expr>,
        /// May be assigned again (`let mut`, a typed local).
        mutable: bool,
    },
    /// `place op= value;`; `op` None for `=`.
    Assign {
        place: Expr,
        op: Option<super::BinOp>,
        value: Expr,
    },
    If {
        cond: Expr,
        then: Box<Stmt>,
        otherwise: Option<Box<Stmt>>,
    },
    Foreach {
        ty: Option<Type>,
        binding: Binding,
        iter: Expr,
        body: Box<Stmt>,
        /// `foreach (let mut x in items)`: each item lent to be changed
        /// (Rust's `iter_mut`); else read only.
        mutable: bool,
        /// `await foreach`: the items of an `IAsyncEnumerable<T>`, each
        /// awaited (in an `async` method).
        awaits: bool,
    },
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        step: Option<Box<Stmt>>,
        body: Box<Stmt>,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    DoWhile {
        body: Box<Stmt>,
        cond: Expr,
    },
    Block(Block),
    /// `name:` (C#'s label): where a `goto name;` in its block (or one
    /// inside it) goes on from.
    Label(Name),
    /// `goto name;`, `goto case value;`, `goto default;`.
    Goto(Goto, Span),
    Break(Span),
    Continue(Span),
    Return(Option<Expr>, Span),
    /// `skip value;` in a patch's `before`: the method ends with it.
    Skip(Option<Expr>, Span),
    /// `yield return value;` (`Some`) or `yield break;` (`None`), in an
    /// iterator.
    Yield(Option<Expr>, Span),
    /// `unsafe { … }`: the threads checks skipped inside (a warning).
    Unsafe(Block, Span),
    /// `checked { … }` (`checked`) or `unchecked { … }`: the block's
    /// whole-number arithmetic panicking on overflow, or wrapping.
    Checked(Block, bool),
    /// `switch (expr) { case p when g: … break; default: … }` (C#'s
    /// statement): the first section a label of takes the value runs; its
    /// `break` leaves the switch.
    Switch {
        expr: Expr,
        sections: Vec<Section>,
    },
    /// A local function (C#'s): a method declared in a block, reaching the
    /// block's locals, called anywhere in it (before it too).
    Function(Box<super::Decl>),
    /// `lock (target) body` or `lock (target, ref let value) body`: a
    /// `Lock<T>` held for `body`, its value lent as `value`.
    Lock {
        target: Expr,
        value: Option<super::Name>,
        body: Box<Stmt>,
        at: Span,
    },
    Expr(Expr),
    /// A block lambda's last expression, without `;`: its value.
    Value(Expr),
}

/// The local functions `stmts` declare.
pub fn local_functions(stmts: &[Stmt]) -> impl Iterator<Item = &super::Method> {
    stmts.iter().filter_map(|s| match s {
        Stmt::Function(d) => match &d.kind {
            super::DeclKind::Method(m) => Some(m),
            _ => None,
        },
        _ => None,
    })
}

/// A `switch` statement's section: its labels (`case p when g:`; `None`:
/// `default:`) and its statements.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub labels: Vec<Option<(super::Pattern, Option<Expr>)>>,
    pub body: Vec<Stmt>,
}
