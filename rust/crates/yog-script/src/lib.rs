//! Yog Script (`.ys`) and Yog Nanite Script (`.yns`): the language a Yog
//! mod is written in (`docs/design/ys.md`, `docs/design/ys-grammar.md`).
//! This crate reads a source into its syntax tree and says what's wrong
//! with it, as Diagnostics placed in the source.

pub mod ast;
mod lexer;
pub mod parser;
mod report;
pub mod token;
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

/// The source `src` of the file `name` read; patches are allowed when it's
/// a `.yns` file.
#[must_use]
pub fn parse(name: &str, src: &str) -> Parsed {
    let nanites = Format::Nanites.is(Path::new(name));
    let mut report = report::Report::new(name, src);
    let tokens = lexer::lex(src, &mut report);
    let file = parser::Parser::new(&tokens, &mut report, nanites).file();
    Parsed {
        file,
        problems: report.out,
    }
}
