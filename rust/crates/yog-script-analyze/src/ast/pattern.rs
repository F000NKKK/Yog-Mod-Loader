//! Patterns (`ys-grammar.md` §3.3) and types (§4).

use super::{BinOp, Expr, Name};
use crate::source::token::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wild(Span),
    /// A literal or a path to a value or a case (`0`, `Mode.Idle`); a
    /// lone path may be a type too: the resolver and the checker tell.
    Value(Expr),
    /// `let e`.
    Let(Name),
    /// `Monster m`, `Monster`.
    Type {
        ty: Type,
        bind: Option<Name>,
    },
    /// `Mode.Charging(let rate)`.
    Variant {
        path: Vec<Name>,
        args: Vec<Pattern>,
    },
    /// `< 5`.
    Relational {
        op: BinOp,
        value: Expr,
    },
    Range {
        from: Expr,
        to: Expr,
    },
    Tuple(Vec<Pattern>),
    /// `{ Health: < 5 }`, `Monster { Health: < 5 }`.
    Property {
        ty: Option<Type>,
        fields: Vec<(Name, Pattern)>,
    },
    Or(Box<Pattern>, Box<Pattern>),
    And(Box<Pattern>, Box<Pattern>),
    Not(Box<Pattern>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    /// `i32`, `List<T>`, `Mode.Charging`.
    Path {
        path: Vec<Name>,
        args: Vec<Type>,
    },
    /// `(i32 X, i32 Y)`.
    Tuple(Vec<(Type, Option<Name>)>),
    Nullable(Box<Type>),
    /// `Charge & Owned`.
    Both(Vec<Type>),
    /// `in 'a T`, `ref 'a T`: a borrow (Rust's `&'a T`, `&'a mut T`).
    Borrow {
        mutable: bool,
        lifetime: Option<Name>,
        ty: Box<Type>,
    },
    /// `'a` among a type's arguments (`Cursor<'a>`).
    Lifetime(Name),
}
