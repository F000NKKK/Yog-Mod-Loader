//! Code the compiler writes for a declaration (.NET's source generators),
//! over the tokens before anything reads them: each `[LoggerMessage(…)]`
//! method's body and the state it logs (`logger_message.rs`).

mod logger_message;
mod signature;
mod template;

use yog_ids::script_problem;
use yog_script_ids::system::{LOGGER, LOGGER_MESSAGE};

use super::calls::{decl_end, read};
use super::tree::{Tree, flatten, tree_at};
use crate::source::token::{Punct, Tok, Token};
use yog_analysis::Report;

/// `tokens` (cut from `text`) with each `[LoggerMessage]` method generated.
pub(super) fn expand(mut tokens: Vec<Token>, text: &str, report: &mut Report<'_>) -> Vec<Token> {
    let mut i = 0;
    while i < tokens.len() {
        let Some((args, after)) = logger_message_at(&tokens, i) else {
            i += 1;
            continue;
        };
        let at = tokens[i].span;
        let end = decl_end(&tokens, after);
        let sig = signature::signature(&tokens[after..end], text);
        let args = signature::args(&args, text);
        let logger = enclosing_logger(&tokens, i);
        let code = sig
            .ok_or_else(|| "`[LoggerMessage]` is for a method".to_string())
            .and_then(|s| logger_message::generate(&s, &args, logger.as_deref()));
        match code {
            Ok(code) => {
                let made = read(&code, at, report);
                i += made.len();
                tokens.splice(i - made.len()..end, made);
            }
            Err(why) => {
                report.error(script_problem::LOGGER_MESSAGE, at, why);
                i = end;
            }
        }
    }
    tokens
}

/// `[LoggerMessage(args)]` at `i`: its arguments' tokens, and where the
/// declaration after it starts.
fn logger_message_at(tokens: &[Token], i: usize) -> Option<(Vec<Token>, usize)> {
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
    if word.tok != Tok::Ident(LOGGER_MESSAGE.to_string()) {
        return None;
    }
    let mut flat = Vec::new();
    flatten(args, &mut flat);
    Some((flat, after))
}

/// The `ILogger` of the class declaring what's at `i`: a field
/// (`private readonly ILogger _logger;`) or its primary constructor's
/// parameter (`class Mover(ILogger logger)`), the name it's reached by.
fn enclosing_logger(tokens: &[Token], i: usize) -> Option<String> {
    let mut depth = 0;
    let open = (0..i).rev().find(|&k| {
        match tokens[k].tok {
            Tok::Punct(Punct::RBrace) => depth += 1,
            Tok::Punct(Punct::LBrace) if depth == 0 => return true,
            Tok::Punct(Punct::LBrace) => depth -= 1,
            _ => {}
        }
        false
    })?;
    let (body, _) = tree_at(tokens, open);
    let Tree::Group { inner, .. } = body else {
        return None;
    };
    let fields = inner.iter().filter_map(|t| match t {
        Tree::Leaf(l) => Some(l.clone()),
        Tree::Group { .. } => None,
    });
    // The header: back to the declaration before it (a `;` or a `}`).
    let head = (0..open)
        .rev()
        .take_while(|&k| {
            !matches!(
                tokens[k].tok,
                Tok::Punct(Punct::Semi | Punct::RBrace | Punct::LBrace)
            )
        })
        .map(|k| tokens[k].clone())
        .collect::<Vec<_>>()
        .into_iter()
        .rev();
    named_logger(head.chain(fields))
}

/// The name after the first `ILogger` (or `ILogger?`) of `tokens`.
fn named_logger(tokens: impl Iterator<Item = Token>) -> Option<String> {
    let mut tokens = tokens.skip_while(|t| t.tok != Tok::Ident(LOGGER.to_string()));
    tokens.next()?;
    tokens
        .find_map(|t| match t.tok {
            Tok::Ident(n) => Some(n),
            Tok::Punct(Punct::Question) => None,
            _ => Some(String::new()),
        })
        .filter(|n| !n.is_empty())
}
