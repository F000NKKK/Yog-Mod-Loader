//! Yog Script (`.ys`) and Yog Nanite Script (`.yns`): the language a Yog
//! mod is written in (`docs/design/ys.md`, `docs/design/ys-grammar.md`).
//! This crate reads a source into its syntax tree, resolves its names and
//! says what's wrong with it, as Diagnostics placed in the source.

mod source;

pub use source::token;

pub mod ast;
mod borrowck;
pub mod index;
mod lexer;
pub mod macros;
pub mod parser;
pub mod resolve;
pub mod system;
mod typeck;
pub mod versions;
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
/// a `.yns` file (read as the newest game version Yog runs on).
#[must_use]
pub fn parse(name: &str, src: &str) -> Parsed {
    let mut report = yog_analysis::Report::new(name, src);
    let file = read(name, src, versions::newest(), &mut report);
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

/// [`check`] of a `.yns` with the game's classes of one of the versions
/// it targets: what `yog build` reports, once a version. A `.ys` reaches
/// Yog, not the game: `game` is ignored for it.
#[must_use]
pub fn check_with(name: &str, src: &str, game: Option<&index::ClassIndex>) -> Checked {
    let mut report = yog_analysis::Report::new(name, src);
    // A `.yns` is read as the version whose classes it's checked with.
    let mc = game.map_or(versions::newest(), |g| g.mc.as_str());
    let file = read(name, src, mc, &mut report);
    let symbols = resolve::resolve(&file, &mut report);
    let game = game.filter(|_| is_nanites(name));
    // Types are checked in what reads and resolves cleanly: a broken tree
    // would only report the same problem again.
    if !report.out.iter().any(yog_data::Diagnostic::is_error) {
        typeck::check(&file, is_nanites(name), game, &mut report);
        borrowck::check(&file, &mut report);
    }
    Checked {
        file,
        symbols,
        problems: report.out,
    }
}

fn read(name: &str, src: &str, mc: &str, report: &mut yog_analysis::Report<'_>) -> ast::File {
    let nanites = is_nanites(name);
    let text = preprocessed(src, mc, nanites, report);
    let tokens = macros::expand(lexer::lex(&text, report), report);
    let file = parser::Parser::new(&tokens, report, nanites).file();
    check_targets(&file, nanites, report);
    file
}

/// `src` as game version `mc` reads it: a `.yns`'s `#if MC >= 1.21`
/// directives (C#'s, by the toolkit's preprocessor, which knows nothing of
/// Yog: it's handed the variable) applied; a `.ys` has none, as it runs on
/// every version unchanged.
fn preprocessed(
    src: &str,
    mc: &str,
    nanites: bool,
    report: &mut yog_analysis::Report<'_>,
) -> String {
    use yog_analysis::preprocess::{Defines, Problem};
    use yog_ids::script_problem::DIRECTIVE;
    let defines: Defines = if nanites {
        [(yog_ids::script_define::GAME.to_string(), mc.to_string())].into()
    } else {
        Defines::new()
    };
    let out = yog_analysis::preprocess::preprocess(src, &defines);
    for (at, problem) in out.problems {
        let span = source::token::Span::new(at.start, at.end);
        let message = match problem {
            Problem::Unmatched => "no `#if` before it, or after `#else`".to_string(),
            Problem::Unterminated => "this `#if` never ends: `#endif`".to_string(),
            Problem::BadCondition(why) => format!("the condition doesn't read: {why}"),
        };
        report.error(DIRECTIVE, span, message);
    }
    if out.directives && !nanites {
        report.error(
            DIRECTIVE,
            source::token::Span::default(),
            "a `.ys` runs on every game version unchanged: directives (`#if`) are for a `.yns`",
        );
    }
    out.text
}

/// Whether the file `name` is Yog Nanite Script (`.yns`): patches allowed,
/// the game's classes reachable.
#[must_use]
pub fn is_nanites(name: &str) -> bool {
    Format::Nanites.is(Path::new(name))
}

/// The game versions a `.yns` says it's for (`targets mc …;`).
#[must_use]
pub fn targets(file: &ast::File) -> Option<&ast::VersionCond> {
    file.decls.iter().find_map(|d| match &d.kind {
        ast::DeclKind::Targets(cond) => Some(cond),
        _ => None,
    })
}

/// A `.yns` names its versions once; a `.ys` doesn't (the mod does).
fn check_targets(file: &ast::File, nanites: bool, report: &mut yog_analysis::Report<'_>) {
    use yog_ids::script_problem::TARGETS;
    let all: Vec<_> = file
        .decls
        .iter()
        .filter(|d| matches!(d.kind, ast::DeclKind::Targets(_)))
        .collect();
    match (nanites, all.as_slice()) {
        (true, []) => {
            let all = yog_ids::game_version::ALL_VALUES;
            let (oldest, newest) = (all.first().unwrap_or(&""), all.last().unwrap_or(&""));
            report.error(
                TARGETS,
                source::token::Span::default(),
                format!("a `.yns` says which game versions its patches are for: `targets mc {oldest}..{newest};`"),
            );
        }
        (true, [_, again, ..]) => report.error(TARGETS, again.span, "`targets` is said once a file"),
        (false, [first, ..]) => report.error(
            TARGETS,
            first.span,
            "`targets` is for `.yns` files: a `.ys` reaches Yog, not the game, so it runs on the mod's every version",
        ),
        _ => {}
    }
}
