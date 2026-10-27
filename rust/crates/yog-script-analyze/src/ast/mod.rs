//! The syntax tree of a Yog Script source (`docs/design/ys-grammar.md`).

mod accessor;
mod decl;
mod expr;
mod generics;
mod indexer;
mod operator;
mod pass;
mod patch;
mod pattern;
mod stmt;
mod walk;

pub use accessor::{accessor_scope, keeps_value, stores_value, value_param};
pub use decl::{
    Accessor, Attribute, Chain, Ctor, Decl, DeclKind, Field, Method, MethodBody, Modifier, Param,
    Prop, TypeHeader, Variant, VersionCond,
};
pub use expr::{
    Arg, Arm, ArrayItem, BinOp, Element, Expr, ExprKind, LambdaBody, LambdaParam, StrSeg, UnOp,
};
pub use generics::{Constraint, Generic, Variance, Where};
pub use indexer::{Indexer, accessor_name};
pub use operator::{Operator, OperatorKind};
pub use pass::Pass;
pub use patch::{Callee, Local, Patch, PatchItem, Point, Selector, Target};
pub use pattern::{Pattern, Type, TypeKind};
pub use stmt::{Binding, Block, Section, Stmt, local_functions};

use crate::source::token::Span;

/// A block lambda's one parameter (`list.Select { it * 2 }`).
pub const IT: &str = "it";

/// A name as written, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

/// A source file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct File {
    pub decls: Vec<Decl>,
}
