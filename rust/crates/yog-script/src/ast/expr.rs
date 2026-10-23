//! Statements, expressions, patterns and types (`ys-grammar.md` §3, §4).

use super::{Name, VersionCond};
use crate::source::token::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        pattern: Pattern,
        ty: Option<Type>,
        value: Expr,
    },
    Var {
        name: Name,
        ty: Option<Type>,
        value: Option<Expr>,
    },
    /// `place op value`; `op` None for `=`.
    Assign {
        place: Expr,
        op: Option<BinOp>,
        value: Expr,
    },
    For {
        pattern: Pattern,
        iter: Expr,
        body: Block,
    },
    While {
        cond: Cond,
        body: Block,
    },
    Loop(Block),
    Break(Span),
    Continue(Span),
    Return(Option<Expr>, Span),
    /// `skip value` in a patch's `before`: the method ends with it.
    Skip(Option<Expr>, Span),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Unit {
        value: String,
        unit: String,
    },
    Str(Vec<StrSeg>),
    RawStr(String),
    Text(String),
    Content {
        kind: String,
        id: String,
    },
    Tag(String),
    Bool(bool),
    None,
    Ident(String),
    SelfValue,
    It,
    /// `mc >= 1.21`: the running game's version.
    Mc(VersionCond),
    Tuple(Vec<Expr>),
    List(Vec<Expr>),
    Map(Vec<(Expr, Expr)>),
    Lambda {
        params: Vec<Name>,
        body: Vec<Stmt>,
    },
    Unary {
        op: UnOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Range {
        from: Box<Expr>,
        to: Box<Expr>,
        inclusive: bool,
    },
    /// `x is T`, `x is T name`, `x !is T`.
    Is {
        expr: Box<Expr>,
        ty: Type,
        bind: Option<Name>,
        negated: bool,
    },
    /// `x as T`, `x as? T`.
    As {
        expr: Box<Expr>,
        ty: Type,
        checked: bool,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
    },
    Index {
        expr: Box<Expr>,
        index: Box<Expr>,
    },
    /// `f<T, U>` before a call: the type arguments.
    TypeArgs {
        expr: Box<Expr>,
        args: Vec<Type>,
    },
    /// `x.name`, `x?.name`.
    Member {
        expr: Box<Expr>,
        name: Name,
        safe: bool,
    },
    If {
        cond: Box<Cond>,
        then: Block,
        otherwise: Option<Box<Expr>>,
    },
    Match {
        expr: Box<Expr>,
        arms: Vec<Arm>,
    },
    Block(Block),
    /// What couldn't be parsed (reported already).
    Error,
}

/// A piece of a string: text, or an interpolated `{expr}`.
#[derive(Debug, Clone, PartialEq)]
pub enum StrSeg {
    Text(String),
    Expr(Expr),
}

/// `if expr` or `if let pattern = expr`.
#[derive(Debug, Clone, PartialEq)]
pub enum Cond {
    Expr(Expr),
    Let { pattern: Box<Pattern>, value: Expr },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub name: Option<Name>,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    Try,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Coalesce,
    Or,
    And,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    In,
    NotIn,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wild(Span),
    /// A literal or a path to a value (`0`, `energy.max`); a single name
    /// binds or names a case: the resolver tells which.
    Value(Expr),
    Range {
        from: Expr,
        to: Expr,
    },
    Is {
        ty: Type,
        bind: Option<Name>,
    },
    Variant {
        path: Vec<Name>,
        args: Vec<Pattern>,
    },
    Tuple(Vec<Pattern>),
    Or(Vec<Pattern>),
    Else(Span),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Path {
        path: Vec<Name>,
        args: Vec<Type>,
    },
    /// `Int in 0..100`.
    Bounded {
        base: Box<Type>,
        range: Box<Expr>,
    },
    Tuple(Vec<Type>),
    Fn {
        params: Vec<Type>,
        ret: Box<Type>,
    },
    Optional(Box<Type>),
}
