//! The syntax tree of a Yog Script source (`docs/design/ys-grammar.md` §2).

mod expr;
mod patch;

pub use expr::{
    Arg, Arm, BinOp, Block, Cond, Expr, ExprKind, Pattern, Stmt, StrSeg, Type, TypeKind, UnOp,
};
pub use patch::{Callee, Local, Method, Patch, PatchItem, Point, Selector};

use crate::token::Span;

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

/// What a declaration is marked with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Server,
    Client,
    Shared,
    Saved,
    Synced,
    Tune,
    Pub,
}

/// A declaration, its documentation and modifiers.
#[derive(Debug, Clone, PartialEq)]
pub struct Decl {
    pub doc: Vec<String>,
    pub modifiers: Vec<Modifier>,
    pub kind: DeclKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeclKind {
    /// `use a.b.{c, d}` / `use a.b as e`.
    Use {
        path: Vec<Name>,
        items: Vec<Name>,
        alias: Option<Name>,
    },
    /// `mod charge { version = "1.0.0" … }`.
    Mod {
        name: Name,
        props: Vec<Prop>,
    },
    Fn(FnDecl),
    Let {
        pattern: Pattern,
        ty: Option<Type>,
        value: Expr,
    },
    Var(VarDecl),
    Const {
        name: Name,
        ty: Option<Type>,
        value: Expr,
    },
    /// `val x => expr`: computed on every read.
    Val {
        name: Name,
        ty: Option<Type>,
        value: Expr,
    },
    /// `unit Energy` / `unit FE = Energy * 1`.
    Unit {
        name: Name,
        value: Option<Expr>,
    },
    Type {
        name: Name,
        generics: Vec<Generic>,
        ty: Type,
    },
    Record {
        name: Name,
        generics: Vec<Generic>,
        fields: Vec<Param>,
        body: Vec<Decl>,
    },
    Union {
        name: Name,
        generics: Vec<Generic>,
        variants: Vec<Variant>,
        body: Vec<Decl>,
    },
    Enum {
        name: Name,
        cases: Vec<Name>,
        body: Vec<Decl>,
    },
    Extend {
        ty: Type,
        adds: Option<Type>,
        body: Vec<Decl>,
    },
    /// `block Charger { … }`, `item Battery { … }`, a mod's own kinds.
    Content {
        kind: Name,
        name: Name,
        base: Option<Type>,
        body: Vec<Decl>,
    },
    /// `recipe Battery shaped { "RIR" … R = @item:… }`.
    Recipe {
        result: Type,
        kind: Name,
        rows: Vec<String>,
        keys: Vec<Prop>,
    },
    /// A property in a content or mod body: `hardness = 2.0`, `tooltip => …`.
    Prop(Prop),
    /// `slots inventory[4] of Battery`.
    Slots {
        name: Name,
        size: Expr,
        of: Option<Type>,
    },
    /// `player state { … }`.
    State {
        scope: Name,
        body: Vec<Decl>,
    },
    Screen {
        name: Name,
        params: Vec<Param>,
        ui: String,
        body: Vec<Decl>,
    },
    /// `action dump() { … }` in a screen: runs on the server.
    Action(FnDecl),
    Command {
        name: String,
        body: Vec<Decl>,
    },
    /// `on player.join(p) { … }`, `on use(p) { … }`.
    On {
        event: Vec<Name>,
        params: Vec<Name>,
        body: Block,
    },
    Test {
        name: String,
        body: Block,
    },
    Version {
        cond: VersionCond,
        then: Vec<Decl>,
        otherwise: Option<Vec<Decl>>,
    },
    Native {
        library: String,
        fns: Vec<FnDecl>,
    },
    Patch(Patch),
}

/// `name = value` (or `name => value`, computed).
#[derive(Debug, Clone, PartialEq)]
pub struct Prop {
    pub name: Name,
    pub value: Expr,
    pub computed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    pub name: Name,
    pub generics: Vec<Generic>,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    pub requires: Option<Expr>,
    /// None: a signature only (`native`).
    pub body: Option<FnBody>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FnBody {
    Block(Block),
    Expr(Expr),
}

/// A parameter (or a record's field): typed except where the grammar lets
/// it be inferred.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub var: bool,
    pub name: Name,
    pub ty: Option<Type>,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarDecl {
    pub name: Name,
    pub ty: Option<Type>,
    pub value: Option<Expr>,
    /// `was power: Int`: what an old save calls it.
    pub was: Option<(Name, Option<Type>)>,
}

/// `T` or `T: Base`.
#[derive(Debug, Clone, PartialEq)]
pub struct Generic {
    pub name: Name,
    pub bound: Option<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub name: Name,
    pub fields: Vec<Param>,
}

/// `mc >= 1.21`, `mc 1.20.1..1.21.4`.
#[derive(Debug, Clone, PartialEq)]
pub enum VersionCond {
    Compare { op: BinOp, version: String },
    Range { from: String, to: String },
}
