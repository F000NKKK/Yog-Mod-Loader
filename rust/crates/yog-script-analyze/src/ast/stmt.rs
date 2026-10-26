//! Blocks and statements (`ys-grammar.md` §3.1).

use super::{Expr, Name, Type};
use crate::source::token::Span;

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
