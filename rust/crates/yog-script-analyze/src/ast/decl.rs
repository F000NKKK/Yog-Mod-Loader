//! Declarations (`ys-grammar.md` §2): the mod, types, members, content,
//! components and systems, events.

use super::{Block, Expr, Generic, Name, Patch, Type, Where};
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
    Protected,
    Abstract,
    Virtual,
    Override,
    Sealed,
    /// `ref struct`: holds borrows, never on the heap.
    Ref,
    /// Its body is elsewhere: a `[Service]` method's, on the bus.
    Extern,
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
    pub generics: Vec<Generic>,
    pub ctor: Option<Vec<Param>>,
    pub bases: Vec<Type>,
    pub constraints: Vec<Where>,
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
    /// `using Grid = Dictionary<Pos, i32>;`.
    Alias {
        name: Name,
        ty: Type,
    },
    /// `mod Charge { Version = "1.0.0"; … }`: its properties, and members —
    /// its constructor runs when the mod loads.
    Mod {
        name: Name,
        body: Vec<Decl>,
    },
    /// `public event Action<Player> Joined;`.
    Event {
        ty: Type,
        name: Name,
    },
    Method(Method),
    /// `delegate R Name<…>(params);`: a type of methods, its signature
    /// what calling one takes and gives (`Invoke`'s).
    Delegate(Method),
    Field(Field),
    /// `Ticks Left { get; set; }`, `Energy Room => …;`.
    Property {
        ty: Type,
        name: Name,
        accessors: Vec<Accessor>,
        value: Option<Expr>,
    },
    /// `T this[i32 index] { get; set; }`: what `x[i]` reads and writes.
    Indexer(super::Indexer),
    /// `static R operator +(A a, B b)`, `static implicit operator R(A a)`.
    Operator(super::Operator),
    Ctor(Ctor),
    /// `class`, `struct`, `record`, `interface`: which is `word`;
    /// `value`: copied by value (a struct, a `record struct`).
    Type {
        word: Name,
        name: Name,
        value: bool,
        header: TypeHeader,
        body: Vec<Decl>,
    },
    /// `enum Mode { Idle, Charging(i32 Rate) }`, `enum Tier : u8 { Wood = 1 }`.
    Enum {
        name: Name,
        generics: Vec<Generic>,
        base: Option<Type>,
        variants: Vec<Variant>,
        body: Vec<Decl>,
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
    Test {
        name: String,
        body: Block,
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

/// `i32 Area(i32 w, i32 h) => w * h;`; `ret` None: `void`.
#[derive(Debug, Clone, PartialEq)]
pub struct Method {
    pub ret: Option<Type>,
    pub name: Name,
    pub generics: Vec<Generic>,
    pub params: Vec<Param>,
    pub constraints: Vec<Where>,
    pub requires: Option<Expr>,
    /// None: a signature only (`native`, an interface's).
    pub body: Option<MethodBody>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MethodBody {
    Block(Block),
    Expr(Expr),
}

/// `saved synced Energy Energy in 0..capacity = 0 was Power: i32;`.
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
    /// `: this(…)` / `: base(…)`: the constructor run first.
    pub chain: Option<(Chain, Vec<super::Arg>)>,
    pub body: Block,
}

/// Which constructor a constructor runs first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    This,
    Base,
}

/// `Player target = source.Player`.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub pass: Option<super::Pass>,
    /// `in 'a T x`: the borrow's lifetime.
    pub lifetime: Option<Name>,
    pub ty: Type,
    pub name: Name,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub name: Name,
    pub fields: Vec<Param>,
    /// `Wood = 1`: its number.
    pub value: Option<Expr>,
}

/// `mc >= 1.21`, `mc 1.20.1..1.21.4`.
#[derive(Debug, Clone, PartialEq)]
pub enum VersionCond {
    Compare { op: super::BinOp, version: String },
    Range { from: String, to: String },
}
