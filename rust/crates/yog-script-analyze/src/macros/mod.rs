//! Declarative macros (`ys.md` §3, Attributes and macros), expanded over
//! the tokens before anything reads the code: `macro Name { rules }` is
//! taken out, each `Name!(…)` replaced by what its first matching rule
//! writes, again and again until no call of a declared macro is left. The
//! built-in macros (`panic!`, …) stay for the parser.

pub mod builtin;
mod calls;
mod fragment;
mod matcher;
mod procs;
mod rules;
mod transcribe;
mod tree;

use std::collections::HashMap;

use yog_ids::script_problem;

use crate::source::token::{Punct, Tok, Token};
use crate::words::r#macro;
use rules::Macro;
use tree::{Tree, closer, tree_at};
use yog_analysis::Report;

pub use procs::ProcMacros;
pub(crate) use transcribe::HYGIENE_MARK;

/// How many expansions one source may make: a macro calling itself
/// forever stops here.
const MAX_EXPANSIONS: usize = 4096;

/// `tokens` (ending with `Eof`, cut from `text`) with the source's macros
/// expanded: its declared ones, then the build's procedural ones.
pub(crate) fn expand(
    mut tokens: Vec<Token>,
    text: &str,
    procs: Option<&dyn ProcMacros>,
    report: &mut Report<'_>,
) -> Vec<Token> {
    let eof = tokens.pop();
    let macros = take_macros(&mut tokens, report);
    if !macros.is_empty() {
        tokens = expand_calls(tokens, &macros, report);
    }
    if let Some(procs) = procs {
        tokens = calls::expand(tokens, text, procs, report);
    }
    tokens.extend(eof);
    tokens
}

/// Takes every `macro Name { … }` out of `tokens`: the macros by name.
fn take_macros(tokens: &mut Vec<Token>, report: &mut Report<'_>) -> HashMap<String, Macro> {
    let mut macros = HashMap::new();
    let mut i = 0;
    while i < tokens.len() {
        let is_decl = matches!(&tokens[i].tok, Tok::Keyword(w) if **w == r#macro::WORD)
            && matches!(tokens.get(i + 1).map(|t| &t.tok), Some(Tok::Ident(_)))
            && tokens
                .get(i + 2)
                .is_some_and(|t| t.tok == Tok::Punct(Punct::LBrace));
        if !is_decl {
            i += 1;
            continue;
        }
        let Tok::Ident(name) = tokens[i + 1].tok.clone() else {
            continue;
        };
        let (body, end) = tree_at(tokens, i + 2);
        if let Tree::Group { inner, .. } = &body {
            macros.insert(name, rules::read(inner, tokens[i].span, report));
        }
        tokens.drain(i..end);
    }
    macros
}

/// Each `Name!( … )` of a declared macro replaced by its expansion.
fn expand_calls(
    mut tokens: Vec<Token>,
    macros: &HashMap<String, Macro>,
    report: &mut Report<'_>,
) -> Vec<Token> {
    let mut made = 0;
    let mut i = 0;
    while i < tokens.len() {
        let Some(m) = call_at(&tokens, i, macros) else {
            i += 1;
            continue;
        };
        let at = tokens[i].span;
        let (args, end) = tree_at(&tokens, i + 2);
        let Tree::Group { inner, .. } = args else {
            break;
        };
        made += 1;
        if made > MAX_EXPANSIONS {
            report.error(
                script_problem::MACRO,
                at,
                "macros expand without end here (a macro calling itself?)",
            );
            break;
        }
        let Some(written) = expansion(m, &inner, at, made, report) else {
            i = end;
            continue;
        };
        tokens.splice(i..end, written);
    }
    tokens
}

/// The declared macro called at `i` (`Name!` then a bracket, no spaces).
fn call_at<'m>(
    tokens: &[Token],
    i: usize,
    macros: &'m HashMap<String, Macro>,
) -> Option<&'m Macro> {
    let Tok::Ident(name) = &tokens[i].tok else {
        return None;
    };
    let bang = tokens.get(i + 1)?;
    let open = tokens.get(i + 2)?;
    let joined = bang.tok == Tok::Punct(Punct::Bang)
        && bang.span.start == tokens[i].span.end
        && open.span.start == bang.span.end
        && closer(&open.tok).is_some();
    if joined { macros.get(name) } else { None }
}

/// What macro `m` writes for a call with `args`, by its first rule that
/// matches them.
fn expansion(
    m: &Macro,
    args: &[Tree],
    at: crate::source::token::Span,
    n: usize,
    report: &mut Report<'_>,
) -> Option<Vec<Token>> {
    for rule in &m.rules {
        if let Some(b) = matcher::matched(&rule.matcher, args, report) {
            let mut w = transcribe::Writer::new(at, report);
            w.write(&rule.body, &b);
            return Some(w.finish(n));
        }
    }
    report.error(
        script_problem::MACRO,
        at,
        "no rule of this macro matches these arguments",
    );
    None
}
