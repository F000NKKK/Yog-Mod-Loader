//! Declarations (`ys-grammar.md` §2): the mod, types, members, content,
//! components and systems, events.

use super::{Block, Expr, Name, Patch, Type};
use crate::source::token::Span;

/// What a declaration is marked with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Public,
    Private,
    Internal,
    Static,
    Const,
    Readonly,
    Async,
    Server,
    Client,
    Shared,
    Saved,
    Synced,
    Tune,
}

/// `[Obsolete("…")]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub path: Vec<Name>,
    pub args: Vec<super::Arg>,
}

/// A declaration, its attributes, documentation and modifiers.
#[derive(Debug, Clone, PartialEq)]
pub struct Decl {
    pub attributes: Vec<Attribute>,
    pub doc: Vec<String>,
    pub modifiers: Vec<Modifier>,
    pub kind: DeclKind,
    pub span: Span,
}

impl Decl {
    pub fn has(&self, m: Modifier) -> bool {
        self.modifiers.contains(&m)
    }
}

/// A type's name and what follows it: generics, its primary constructor,
/// what it implements, the constraints, the components it has.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TypeHeader {
    pub generics: Vec<Name>,
    pub ctor: Option<Vec<Param>>,
    pub bases: Vec<Type>,
    pub constraints: Vec<(Name, Vec<Type>)>,
    pub has: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeclKind {
    /// `using Yog.Ui;`, `using static Yog.Chat;`, `global using …;`.
    Using {
        path: Vec<Name>,
        global: bool,
        statics: bool,
    },
    /// `namespace A.B;` (the rest of the file: `body` None) or `namespace A.B { … }`.
    Namespace {
        path: Vec<Name>,
        body: Option<Vec<Decl>>,
    },
    /// `using Grid = Dictionary<Pos, int>;`.
    Alias {
        name: Name,
        ty: Type,
    },
    /// `mod Charge { Version = "1.0.0"; … }`.
    Mod {
        name: Name,
        props: Vec<Prop>,
    },
    Method(Method),
    Field(Field),
    /// `Ticks Left { get; set; }`, `Energy Room => …;`.
    Property {
        ty: Type,
        name: Name,
        accessors: Vec<Accessor>,
        value: Option<Expr>,
    },
    Ctor(Ctor),
    /// `class`, `record`, `interface`: which is `word`.
    Type {
        word: Name,
        name: Name,
        header: TypeHeader,
        body: Vec<Decl>,
    },
    Union {
        name: Name,
        generics: Vec<Name>,
        variants: Vec<Variant>,
        body: Vec<Decl>,
    },
    Enum {
        name: Name,
        cases: Vec<Name>,
    },
    /// `unit Energy;` / `unit FE = Energy * 1;`.
    Unit {
        name: Name,
        value: Option<Expr>,
    },
    /// `block Charger has Charge(1000) { … }`, `recipe Battery : Shaped { … }`,
    /// a mod's own kinds.
    Content {
        kind: Name,
        name: Name,
        header: TypeHeader,
        body: Vec<Decl>,
    },
    Component {
        name: Name,
        header: TypeHeader,
        body: Vec<Decl>,
    },
    /// `server system Solar(Charge charge, Block block) every 1t { … }`.
    System {
        name: Name,
        params: Vec<Param>,
        every: Option<Expr>,
        body: Block,
    },
    Extend {
        ty: Type,
        header: TypeHeader,
        body: Vec<Decl>,
    },
    /// `Hardness = 2.0f;` in a content or mod body.
    Prop(Prop),
    Screen {
        name: Name,
        header: TypeHeader,
        ui: String,
        body: Vec<Decl>,
    },
    Command {
        name: String,
        body: Vec<Decl>,
    },
    /// `on Player.Join(Player player) { … }`, `on Use(Player player) { … }`.
    On {
        event: Vec<Name>,
        params: Vec<Param>,
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
        methods: Vec<Method>,
    },
    Patch(Patch),
    /// `targets mc 1.20.1..26.2;`: the game versions a `.yns` is for.
    Targets(VersionCond),
}

/// `Name = value;`.
#[derive(Debug, Clone, PartialEq)]
pub struct Prop {
    pub name: Name,
    pub value: Expr,
}

/// `int Area(int w, int h) => w * h;`; `ret` None: `void`.
#[derive(Debug, Clone, PartialEq)]
pub struct Method {
    pub ret: Option<Type>,
    pub name: Name,
    pub generics: Vec<Name>,
    pub params: Vec<Param>,
    pub constraints: Vec<(Name, Vec<Type>)>,
    pub requires: Option<Expr>,
    /// None: a signature only (`native`, an interface's).
    pub body: Option<MethodBody>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MethodBody {
    Block(Block),
    Expr(Expr),
}

/// `saved synced Energy Energy in 0..capacity = 0 was Power: int;`.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub ty: Type,
    pub name: Name,
    pub bound: Option<Expr>,
    pub value: Option<Expr>,
    /// What an old save calls it.
    pub was: Option<(Name, Option<Type>)>,
}

/// `get;`, `set { … }`, `init;`.
#[derive(Debug, Clone, PartialEq)]
pub struct Accessor {
    pub name: Name,
    pub body: Option<MethodBody>,
}

/// `public Timer() : this(20t) { … }`.
#[derive(Debug, Clone, PartialEq)]
pub struct Ctor {
    pub name: Name,
    pub params: Vec<Param>,
    pub this_args: Option<Vec<super::Arg>>,
    pub body: Block,
}

/// `Player target = source.Player`.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub ty: Type,
    pub name: Name,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub name: Name,
    pub fields: Vec<Param>,
}

/// `mc >= 1.21`, `mc 1.20.1..1.21.4`.
#[derive(Debug, Clone, PartialEq)]
pub enum VersionCond {
    Compare { op: super::BinOp, version: String },
    Range { from: String, to: String },
}
