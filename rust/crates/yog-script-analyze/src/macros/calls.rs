//! Procedural macros at work over the tokens: a `Name!(…)` call no
//! declared or built-in macro takes, and each `[Derive(…)]` on a type,
//! handed to the build's macros; what they give read in.

use yog_ids::script_problem;

use super::procs::ProcMacros;
use super::tree::{Tree, tree_at};
use crate::source::token::{Punct, Span, Tok, Token};
use yog_analysis::Report;
use yog_script_ids::system::DERIVE;

/// How many procedural expansions one source may make.
const MAX_CALLS: usize = 1024;

pub(super) fn expand(
    mut tokens: Vec<Token>,
    text: &str,
    procs: &dyn ProcMacros,
    report: &mut Report<'_>,
) -> Vec<Token> {
    let mut made = 0;
    let mut i = 0;
    while i < tokens.len() && made < MAX_CALLS {
        if let Some(name) = call_at(&tokens, i) {
            let (args, end) = tree_at(&tokens, i + 2);
            let at = tokens[i].span;
            match procs.call(&name, inside(&args, text)) {
                Some(Ok(code)) => {
                    tokens.splice(i..end, read(&code, at, report));
                    made += 1;
                }
                Some(Err(why)) => {
                    report.error(
                        script_problem::MACRO,
                        at,
                        format!("`{name}!` failed: {why}"),
                    );
                    i = end;
                }
                None => i += 1,
            }
            continue;
        }
        if let Some((end, code)) = attribute_macro(&tokens, i, text, procs, report) {
            tokens.splice(i..end, code);
            made += 1;
            continue;
        }
        if let Some((names, after)) = derive_at(&tokens, i) {
            let end = decl_end(&tokens, after);
            let item = slice(
                text,
                tokens.get(after).map(|t| t.span),
                tokens.get(end - 1).map(|t| t.span),
            );
            let mut added = Vec::new();
            for (name, at) in names {
                match procs.derive(&name, item) {
                    Some(Ok(code)) => added.extend(read(&code, at, report)),
                    Some(Err(why)) => report.error(
                        script_problem::MACRO,
                        at,
                        format!("deriving `{name}` failed: {why}"),
                    ),
                    None => report.error(
                        script_problem::MACRO,
                        at,
                        format!("no derive macro `{name}`"),
                    ),
                }
                made += 1;
            }
            tokens.splice(end..end, added);
            i = after;
            continue;
        }
        i += 1;
    }
    tokens
}

/// The name of a procedural macro called at `i`: `Name!(` with nothing
/// between, not a built-in.
fn call_at(tokens: &[Token], i: usize) -> Option<String> {
    let Tok::Ident(name) = &tokens[i].tok else {
        return None;
    };
    let (bang, open) = (tokens.get(i + 1)?, tokens.get(i + 2)?);
    let joined = bang.tok == Tok::Punct(Punct::Bang)
        && open.tok == Tok::Punct(Punct::LParen)
        && bang.span.start == tokens[i].span.end
        && open.span.start == bang.span.end;
    (joined && super::builtin::named(name).is_none()).then(|| name.clone())
}

/// `[Derive(A, B)]` at `i`: the macros it names (and where), and where
/// the declaration after it starts.
fn derive_at(tokens: &[Token], i: usize) -> Option<(Vec<(String, Span)>, usize)> {
    if tokens[i].tok != Tok::Punct(Punct::LBracket) {
        return None;
    }
    let (attr, after) = tree_at(tokens, i);
    let Tree::Group { inner, .. } = attr else {
        return None;
    };
    let [Tree::Leaf(word), Tree::Group { inner: args, .. }] = inner.as_slice() else {
        return None;
    };
    if word.tok != Tok::Ident(DERIVE.to_string()) {
        return None;
    }
    let names = args
        .iter()
        .filter_map(|a| match a {
            Tree::Leaf(Token {
                tok: Tok::Ident(n),
                span,
            }) => Some((n.clone(), *span)),
            _ => None,
        })
        .collect();
    Some((names, after))
}

/// An attribute macro's `[Name(args)]` at `i`: where its declaration ends
/// and the code that takes their place.
fn attribute_macro(
    tokens: &[Token],
    i: usize,
    text: &str,
    procs: &dyn ProcMacros,
    report: &mut Report<'_>,
) -> Option<(usize, Vec<Token>)> {
    if tokens[i].tok != Tok::Punct(Punct::LBracket) {
        return None;
    }
    let (attr, after) = tree_at(tokens, i);
    let Tree::Group { inner, .. } = &attr else {
        return None;
    };
    let (
        Some(Tree::Leaf(Token {
            tok: Tok::Ident(name),
            span,
        })),
        args,
    ) = (inner.first(), inner.get(1))
    else {
        return None;
    };
    if name == DERIVE {
        return None;
    }
    let end = decl_end(tokens, after);
    let item = slice(
        text,
        tokens.get(after).map(|t| t.span),
        tokens.get(end - 1).map(|t| t.span),
    );
    let args = args.map_or("", |a| inside(a, text));
    match procs.attribute(name, args, item)? {
        Ok(code) => Some((end, read(&code, *span, report))),
        Err(why) => {
            report.error(
                script_problem::MACRO,
                *span,
                format!("`[{name}]` failed: {why}"),
            );
            None
        }
    }
}

/// Where the declaration starting at `from` ends: after its body (`{ … }`)
/// or its `;`.
fn decl_end(tokens: &[Token], from: usize) -> usize {
    let mut i = from;
    while i < tokens.len() {
        let (t, next) = tree_at(tokens, i);
        match t {
            Tree::Group { open, .. } if open.tok == Tok::Punct(Punct::LBrace) => return next,
            Tree::Leaf(semi) if semi.tok == Tok::Punct(Punct::Semi) => return next,
            _ => i = next,
        }
    }
    i
}

/// What a bracketed group holds, as the source wrote it.
fn inside<'t>(group: &Tree, text: &'t str) -> &'t str {
    match group {
        Tree::Group { open, close, .. } => slice_between(text, open.span.end, close.span.start),
        Tree::Leaf(_) => "",
    }
}

fn slice(text: &str, from: Option<Span>, to: Option<Span>) -> &str {
    match (from, to) {
        (Some(a), Some(b)) => slice_between(text, a.start, b.end),
        _ => "",
    }
}

fn slice_between(text: &str, from: u32, to: u32) -> &str {
    text.get(from as usize..to as usize).unwrap_or_default()
}

/// A macro's code read into tokens, each placed at the call.
fn read(code: &str, at: Span, report: &mut Report<'_>) -> Vec<Token> {
    let mut scratch = report.scratch();
    let mut tokens = crate::lexer::lex(code, &mut scratch);
    tokens.pop();
    if let Some(problem) = scratch.out.first() {
        report.error(
            script_problem::MACRO,
            at,
            format!("a macro's code doesn't read: {}", problem.message),
        );
    }
    for t in &mut tokens {
        t.span = at;
    }
    tokens
}
