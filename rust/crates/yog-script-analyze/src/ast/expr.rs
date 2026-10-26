//! Expressions (`ys-grammar.md` §3.2–3.4).

use super::{Name, Pattern, Stmt, Type, VersionCond};
use crate::source::token::Span;

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
    Char(char),
    Text(String),
    Content {
        kind: String,
        id: String,
    },
    Tag(String),
    Bool(bool),
    Null,
    Ident(String),
    This,
    /// `base`: `this` as its base class (`base.Tick()`).
    Base,
    It,
    /// `out let x`, `out i32 x`: a local declared where a method gives it.
    Declare {
        ty: Option<Type>,
        name: Name,
        mutable: bool,
    },
    /// `typeof(T)`: its `System.Type`.
    TypeOf(Type),
    /// `nameof(x.Y)`: its last name, as a string.
    NameOf(Box<Expr>),
    /// `default`, `default(T)`: the type's zero value.
    Default(Option<Type>),
    /// `mc >= 1.21`: the running game's version.
    Mc(VersionCond),
    /// `(a, b)`, `(X: 1, Y: 2)`: a tuple, its items' names where written.
    Tuple {
        items: Vec<Expr>,
        names: Vec<Option<Name>>,
    },
    /// `[a, b]`, `[k => v]`, `[..xs]`: a list, a set or a map by its target.
    Collection(Vec<Element>),
    /// `new Pos(1, 2, 3)`, `new(4)`, `new Battery { Count = 2 }`.
    New {
        ty: Option<Type>,
        args: Option<Vec<Arg>>,
        init: Vec<(Name, Expr)>,
        /// A collection initializer's items, each a statement on the object
        /// made (`members::MADE`): `$made.Add(1);`, `$made.Add(k, v);`,
        /// `$made[k] = v;`.
        items: Vec<super::Stmt>,
    },
    /// `new i32[n]`, `new i32[2, 3]`, `new i32[3][]`, `new[] { 1, 2 }`,
    /// `new i32[,] { { 1, 2 }, { 3, 4 } }`: an array made. `elem` is its
    /// items' type (none: the items' own), `rank` its dimensions.
    NewArray {
        elem: Option<Type>,
        rank: usize,
        sizes: Vec<Expr>,
        init: Option<Vec<ArrayItem>>,
    },
    /// `x => …`, `(a, b) => …`, `{ … }` after a call (`it` its parameter).
    Lambda {
        params: Vec<LambdaParam>,
        body: LambdaBody,
        /// `async x => …`: it awaits; it gives a `Task`.
        asynchronous: bool,
    },
    Unary {
        op: UnOp,
        expr: Box<Expr>,
    },
    /// `(i32)x`.
    Cast {
        ty: Type,
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
    /// `x is Monster m`, `x is not null`.
    Is {
        expr: Box<Expr>,
        pattern: Box<Pattern>,
    },
    /// `x as T`: `null` when it isn't one.
    As {
        expr: Box<Expr>,
        ty: Type,
    },
    /// `x has Charge`.
    Has {
        expr: Box<Expr>,
        ty: Type,
    },
    /// `c ? a : b`.
    Conditional {
        cond: Box<Expr>,
        then: Box<Expr>,
        otherwise: Box<Expr>,
    },
    /// `x with { Y = 64 }`: a copy, members changed.
    With {
        expr: Box<Expr>,
        inits: Vec<(Name, Expr)>,
    },
    /// `x switch { … }`.
    Switch {
        expr: Box<Expr>,
        arms: Vec<Arm>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
    },
    /// `a[i]`, `grid[x, y]`: an indexer's or an array's element.
    Index {
        expr: Box<Expr>,
        args: Vec<Expr>,
    },
    /// `f<T, U>` before a call: the type arguments.
    TypeArgs {
        expr: Box<Expr>,
        args: Vec<Type>,
    },
    /// `x.Name`, `x?.Name`.
    Member {
        expr: Box<Expr>,
        name: Name,
        safe: bool,
    },
    /// `panic!(…)`: a built-in macro's call (a declared macro's is
    /// expanded before the parser sees it).
    Macro {
        name: Name,
        args: Vec<Expr>,
    },
    /// `move x`: `x` handed on, gone after.
    Move(Box<Expr>),
    /// `x!`: asserted not null.
    NotNull(Box<Expr>),
    /// What couldn't be parsed (reported already).
    Error,
}

/// A piece of a string: text, or an interpolated `{expr}`.
#[derive(Debug, Clone, PartialEq)]
pub enum StrSeg {
    Text(String),
    Expr(Expr),
}

/// An item of an array's initializer (`new[] { … }`).
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayItem {
    Value(Expr),
    /// `{ … }` of a dimension inside (`new i32[,] { { 1, 2 } }`).
    Nested(Vec<ArrayItem>),
}

impl ArrayItem {
    /// Each value inside, however deep.
    pub fn each_value<'e>(&'e self, f: &mut impl FnMut(&'e Expr)) {
        match self {
            ArrayItem::Value(e) => f(e),
            ArrayItem::Nested(items) => items.iter().for_each(|i| i.each_value(f)),
        }
    }
}

/// An element of a collection expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    Item(Expr),
    Entry(Expr, Expr),
    Spread(Expr),
}

/// A lambda's parameter: typed or not.
#[derive(Debug, Clone, PartialEq)]
pub struct LambdaParam {
    pub ty: Option<Type>,
    pub name: Name,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LambdaBody {
    Expr(Box<Expr>),
    Block(Vec<Stmt>),
}

/// `value` or `name: value`.
#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub name: Option<Name>,
    pub pass: Option<super::Pass>,
    pub value: Expr,
}

/// `pattern when guard => value`.
#[derive(Debug, Clone, PartialEq)]
pub struct Arm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub value: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    /// `~x`: an integer's bits flipped.
    BitNot,
    /// `++x` / `--x`: the place stepped, its new value.
    Increment,
    Decrement,
    /// `x++` / `x--`: the place stepped, its old value.
    PostIncrement,
    PostDecrement,
    Try,
    Await,
}

impl UnOp {
    /// Whether it steps a place (`++`, `--`, either side).
    pub fn steps(self) -> bool {
        matches!(
            self,
            UnOp::Increment | UnOp::Decrement | UnOp::PostIncrement | UnOp::PostDecrement
        )
    }

    /// The operator a type declares for it: `x++` is `operator ++`, as `++x`.
    pub fn declared(self) -> UnOp {
        match self {
            UnOp::PostIncrement => UnOp::Increment,
            UnOp::PostDecrement => UnOp::Decrement,
            op => op,
        }
    }
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
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    /// `&`, `|`, `^`: an integer's bits, or two `bool`s both evaluated.
    BitAnd,
    BitOr,
    BitXor,
    /// `<<`, `>>`: an integer's bits shifted by a count.
    Shl,
    Shr,
}
