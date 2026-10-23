//! Yog Script (`.ys`) and Yog Nanite Script (`.yns`): the language a Yog
//! mod is written in (`docs/design/ys.md`, `docs/design/ys-grammar.md`).
//! This crate reads a source into its syntax tree, resolves its names and
//! says what's wrong with it, as Diagnostics placed in the source.

mod source;

pub use source::token;

pub mod ast;
pub mod index;
mod lexer;
pub mod parser;
pub mod resolve;
mod typeck;
pub mod words;

use std::path::Path;

use yog_data::Diagnostic;
use yog_ids::Format;

/// A source read: its tree (what could be read of it) and its problems.
#[derive(Debug)]
pub struct Parsed {
    pub file: ast::File,
    pub problems: Vec<Diagnostic>,
}

/// A source checked: its tree, what its names stand for, its problems.
#[derive(Debug)]
pub struct Checked {
    pub file: ast::File,
    pub symbols: resolve::Symbols,
    pub problems: Vec<Diagnostic>,
}

/// The source `src` of the file `name` read; patches are allowed when it's
/// a `.yns` file.
#[must_use]
pub fn parse(name: &str, src: &str) -> Parsed {
    let mut report = yog_analysis::Report::new(name, src);
    let file = read(name, src, &mut report);
    Parsed {
        file,
        problems: report.out,
    }
}

/// The source `src` of the file `name` read, its names resolved and its
/// types checked (the game's unknown): what the editors report.
#[must_use]
pub fn check(name: &str, src: &str) -> Checked {
    check_with(name, src, None)
}

/// [`check`] with the game's classes of a targeted version: what `yog
/// build` reports, once a version.
#[must_use]
pub fn check_with(name: &str, src: &str, game: Option<&index::ClassIndex>) -> Checked {
    let mut report = yog_analysis::Report::new(name, src);
    let file = read(name, src, &mut report);
    let symbols = resolve::resolve(&file, &mut report);
    // Types are checked in what reads and resolves cleanly: a broken tree
    // would only report the same problem again.
    if !report.out.iter().any(yog_data::Diagnostic::is_error) {
        typeck::check(&file, game, &mut report);
    }
    Checked {
        file,
        symbols,
        problems: report.out,
    }
}

fn read(name: &str, src: &str, report: &mut yog_analysis::Report<'_>) -> ast::File {
    let nanites = Format::Nanites.is(Path::new(name));
    let tokens = lexer::lex(src, report);
    parser::Parser::new(&tokens, report, nanites).file()
}
