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
    /// Starts an expression (a literal, a prefix, `if`, `match`).
    Prefix {
        starts: Starts,
        parse: fn(&mut Parser) -> PResult<Expr>,
    },
    /// Joins the expression before it with what follows; `negated` when
    /// written `!word` (`!is`, `!in`).
    Infix {
        level: Level,
        parse: fn(&mut Parser, Expr, bool) -> PResult<Expr>,
        negatable: bool,
    },
    /// Marks the declaration after it.
    Modifier(Modifier),
    /// Starts an item of a patch (`before`, `add`, …).
    PatchItem {
        parse: fn(&mut Parser) -> PResult<PatchItem>,
    },
    /// Starts a point inside a patched method (`around`, `at`, …).
    Point {
        parse: fn(&mut Parser) -> PResult<Point>,
    },
    /// Only part of another word's form (`else`, `was`, `of`).
    Part,
}

/// Binding strength of infix forms, loosest first (`ys-grammar.md` §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Coalesce,
    Or,
    And,
    Compare,
    Range,
    Sum,
    Product,
    Convert,
}

impl Word {
    /// Its role of the kind `pick` finds.
    pub fn role<T>(&self, pick: impl Fn(&Role) -> Option<T>) -> Option<T> {
        self.roles.iter().find_map(pick)
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
        $($(#[path = $path])? pub(crate) mod $file;)*

        /// Every word, in the order the parser offers them a turn.
        pub static ALL: &[&Word] = &[$(&$file::WORD),*];
    };
}

words! {
    // Files and declarations.
    // `mod.rs` and `self` can't name a module file: theirs say `_word`.
    r#use, module = "words/mod_word.rs", r#fn, r#let, r#var, val, r#const, r#type, unit, record, union, r#enum,
    extend, on, test, native, version, recipe, screen, command, state, action, slots,
    patch,
    // Parts of declarations.
    was, requires, ui, of, mc,
    // Modifiers.
    server, client, shared, saved, synced, tune, r#pub,
    // Statements.
    r#for, r#while, r#loop, r#break, r#continue, r#return, skip,
    // Expressions.
    r#if, r#else, r#match, r#try, r#true, r#false, none, this = "words/self_word.rs", it, not, and, or, is, r#as, r#in,
    // Patches: items, points, selectors.
    add, before, after, inside, replace, read, write, priority, uses, local,
    around, at, call, new, constant, first, last, r#where,
}
