//! Blocks and statements (`ys-grammar.md` §3.1).

use super::{Expr, Name, Type};
use crate::source::token::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

/// What a local declaration or a `foreach` binds: one name or a
/// deconstruction (`var (a, b)`).
#[derive(Debug, Clone, PartialEq)]
pub enum Binding {
    One(Name),
    Tuple(Vec<Name>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `var x = …;`, `int x;`, `var (a, b) = …;`; `ty` None: `var`.
    Local {
        ty: Option<Type>,
        binding: Binding,
        value: Option<Expr>,
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
    Expr(Expr),
    /// A block lambda's last expression, without `;`: its value.
    Value(Expr),
}
