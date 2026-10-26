//! The words of Yog Script. Each word is one file under `words/`: how it's
//! written, what it means (shown by editors) and how it parses. The parser
//! core only hands each word its turn; this file only lists them.

use std::fmt;

use crate::ast::{DeclKind, Expr, Modifier, PatchItem, Point, Stmt};
use crate::parser::{PResult, Parser};

/// One word of the language.
pub struct Word {
    /// How it's written.
    pub text: &'static str,
    /// What it means, for editors (hover, completion).
    pub doc: &'static str,
    /// Reserved everywhere (a keyword); otherwise a name where its forms
    /// don't apply.
    pub reserved: bool,
    /// What it does where it stands.
    pub roles: &'static [Role],
}

/// Whether a form starts here, beyond the word itself being here.
pub type Starts = Option<fn(&Parser) -> bool>;

/// What a word does where the parser meets it.
#[derive(Clone, Copy)]
pub enum Role {
    /// Starts a declaration.
    Decl {
        starts: Starts,
        parse: fn(&mut Parser) -> PResult<DeclKind>,
    },
    /// Starts a statement.
    Stmt {
        parse: fn(&mut Parser) -> PResult<Stmt>,
    },
    /// Starts an expression (a literal, a prefix, `new`).
    Prefix {
        starts: Starts,
        parse: fn(&mut Parser) -> PResult<Expr>,
    },
    /// Joins the expression before it with what follows.
    Infix {
        level: Level,
        parse: fn(&mut Parser, Expr) -> PResult<Expr>,
    },
    /// Marks the declaration after it.
    Modifier(Modifier),
    /// A built-in type (`i32`, `string`, `void`).
    BuiltinType,
    /// Starts an item of a patch (`before`, `add`, …).
    PatchItem {
        parse: fn(&mut Parser) -> PResult<PatchItem>,
    },
    /// Starts a point inside a patched method (`around`, `at`, …).
    Point {
        parse: fn(&mut Parser) -> PResult<Point>,
    },
    /// Only part of another word's form (`else`, `was`, `when`).
    Part,
}

/// Binding strength of infix forms, loosest first (`ys-grammar.md` §3.2).
/// Lambdas and `c ? a : b` are looser still; prefixes and postfixes tighter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Coalesce,
    Or,
    And,
    BitOr,
    BitXor,
    BitAnd,
    Equality,
    Compare,
    Range,
    Shift,
    Sum,
    Product,
    Switch,
}

impl Word {
    /// Its role of the kind `pick` finds.
    pub fn role<T>(&self, pick: impl Fn(&Role) -> Option<T>) -> Option<T> {
        self.roles.iter().find_map(pick)
    }

    /// Whether it names a built-in type.
    pub fn is_type(&self) -> bool {
        self.roles.iter().any(|r| matches!(r, Role::BuiltinType))
    }
}

impl PartialEq for Word {
    fn eq(&self, other: &Word) -> bool {
        self.text == other.text
    }
}

impl fmt::Debug for Word {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}`", self.text)
    }
}

/// The reserved word written `text`.
pub fn reserved(text: &str) -> Option<&'static Word> {
    ALL.iter().copied().find(|w| w.reserved && w.text == text)
}

/// The word written `text`, reserved or not.
pub fn named(text: &str) -> Option<&'static Word> {
    ALL.iter().copied().find(|w| w.text == text)
}

macro_rules! words {
    ($($file:ident $(= $path:literal)?),* $(,)?) => {
        $($(#[path = $path])? pub mod $file;)*

        /// Every word, in the order the parser offers them a turn.
        pub static ALL: &[&Word] = &[$(&$file::WORD),*];
    };
}

words! {
    // Files and declarations. `mod.rs` can't name a module file: its says `_word`.
    using, global, namespace, module = "words/mod_word.rs", r#macro, class, r#struct, record, interface, r#enum, unit,
    component, system, extend, event, delegate, test, screen, command, patch, targets,
    // Parts of declarations.
    was, requires, every, has, params, r#where, when, get, set, init, value, field, operator, implicit, explicit, mc, out,
    // Modifiers.
    public, private, internal, protected, r#static, r#const, readonly, r#async,
    r#abstract, r#virtual, r#override, sealed, r#extern,
    server, client, shared, saved, synced, tune,
    // Built-in types.
    void, i8, i16, i32, i64, u8, u16, u32, u64, f32, f64, decimal, bool, char, string, object,
    dynamic,
    // Statements.
    r#let, r#mut, r#if, r#else, foreach, r#for, r#while, r#do, r#break, r#continue, r#return, skip, r#yield, lock,
    // Expressions.
    new, this, base, r#ref, r#move, r#typeof, nameof, default, r#true, r#false, null, r#await, r#try, is, r#as, r#in, switch, with, and, or, not,
    // Patches: items, points, selectors.
    add, before, after, inside, replace, read, write, priority, uses, local,
    around, at, call, constant, first, last,
    // Queries.
    from, select, group, by, orderby, ascending, descending, into, join, on, equals,
}
