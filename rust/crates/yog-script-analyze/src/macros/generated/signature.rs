//! A `[LoggerMessage]` method as written, read off its tokens: its
//! modifiers, name and parameters (each its type as written), and the
//! attribute's arguments.

use crate::macros::tree::{Tree, tree_at};
use crate::source::token::{Punct, StrPart, Tok, Token};
use crate::words::{Role, partial, void};

/// A parameter: its type as written (`ILogger`, `i32`), its name.
pub(super) struct Param {
    pub ty: String,
    pub name: String,
}

/// A method's head: `public static partial void Moved(ILogger logger, i32 count);`.
pub(super) struct Signature {
    /// Its modifiers as written, `partial` left out.
    pub modifiers: Vec<String>,
    pub partial: bool,
    pub void: bool,
    pub name: String,
    pub params: Vec<Param>,
}

/// The source text of `tokens` (from the first's start to the last's end).
pub(super) fn written<'t>(tokens: &[Token], text: &'t str) -> &'t str {
    match (tokens.first(), tokens.last()) {
        (Some(a), Some(b)) => text
            .get(a.span.start as usize..b.span.end as usize)
            .unwrap_or_default(),
        _ => "",
    }
}

/// The method declared by `tokens` (from its first modifier to its `;`).
pub(super) fn signature(tokens: &[Token], text: &str) -> Option<Signature> {
    let mut modifiers = Vec::new();
    let mut partial = false;
    let mut i = 0;
    while let Some(Tok::Keyword(w)) = tokens.get(i).map(|t| &t.tok) {
        if !w.roles.iter().any(|r| matches!(r, Role::Modifier(_))) {
            break;
        }
        if **w == partial::WORD {
            partial = true;
        } else {
            modifiers.push(w.text.to_string());
        }
        i += 1;
    }
    let void = matches!(&tokens.get(i)?.tok, Tok::Keyword(w) if **w == void::WORD);
    let open = (i..tokens.len()).find(|&k| tokens[k].tok == Tok::Punct(Punct::LParen))?;
    let Tok::Ident(name) = &tokens.get(open.checked_sub(1)?)?.tok else {
        return None;
    };
    let (group, _) = tree_at(tokens, open);
    let Tree::Group { inner, .. } = group else {
        return None;
    };
    let mut flat = Vec::new();
    crate::macros::tree::flatten(&inner, &mut flat);
    let params = split(&flat).iter().filter_map(|p| param(p, text)).collect();
    Some(Signature {
        modifiers,
        partial,
        void,
        name: name.clone(),
        params,
    })
}

/// A parameter's tokens: its name the last, its type what's before.
fn param(tokens: &[Token], text: &str) -> Option<Param> {
    let (last, before) = tokens.split_last()?;
    let Tok::Ident(name) = &last.tok else {
        return None;
    };
    Some(Param {
        ty: written(before, text).to_string(),
        name: name.clone(),
    })
}

/// `tokens` split at their commas outside brackets and `<…>`.
pub(super) fn split(tokens: &[Token]) -> Vec<&[Token]> {
    let mut out = Vec::new();
    let (mut depth, mut from) = (0i32, 0);
    for (k, t) in tokens.iter().enumerate() {
        match t.tok {
            Tok::Punct(Punct::Lt | Punct::LParen | Punct::LBracket | Punct::LBrace) => depth += 1,
            Tok::Punct(Punct::Gt | Punct::RParen | Punct::RBracket | Punct::RBrace) => depth -= 1,
            Tok::Punct(Punct::Comma) if depth == 0 => {
                out.push(&tokens[from..k]);
                from = k + 1;
            }
            _ => {}
        }
    }
    if from < tokens.len() {
        out.push(&tokens[from..]);
    }
    out
}

/// The `[LoggerMessage(…)]`'s arguments: by name (`level: …`), or by what
/// they are (a whole number the event id, a string the message, else the level).
#[derive(Default)]
pub(super) struct Args {
    pub event_id: Option<String>,
    pub level: Option<String>,
    pub message: Option<String>,
}

pub(super) fn args(tokens: &[Token], text: &str) -> Args {
    use yog_script_ids::system::{
        LOGGER_MESSAGE_EVENT_ID, LOGGER_MESSAGE_LEVEL, LOGGER_MESSAGE_TEXT,
    };
    let mut out = Args::default();
    for arg in split(tokens) {
        let (named, value) = match arg {
            [
                Token {
                    tok: Tok::Ident(n), ..
                },
                Token {
                    tok: Tok::Punct(Punct::Colon),
                    ..
                },
                rest @ ..,
            ] => (Some(n.as_str()), rest),
            _ => (None, arg),
        };
        let template = match value {
            [
                Token {
                    tok: Tok::Str(parts),
                    ..
                },
            ] => Some(template(parts, text)),
            [
                Token {
                    tok: Tok::RawStr(s),
                    ..
                },
            ] => Some(s.clone()),
            _ => None,
        };
        match (named, template) {
            (Some(LOGGER_MESSAGE_TEXT), Some(t)) | (None, Some(t)) => out.message = Some(t),
            (Some(LOGGER_MESSAGE_EVENT_ID), _) => {
                out.event_id = Some(written(value, text).to_string())
            }
            (None, _)
                if matches!(
                    value,
                    [Token {
                        tok: Tok::Int(_),
                        ..
                    }]
                ) =>
            {
                out.event_id = Some(written(value, text).to_string());
            }
            (Some(LOGGER_MESSAGE_LEVEL) | None, _) => {
                out.level = Some(written(value, text).to_string())
            }
            _ => {}
        }
    }
    out
}

/// A string written for the template, as the template it is: its text's
/// braces doubled, its holes as written.
fn template(parts: &[StrPart], text: &str) -> String {
    use crate::lexer::string::{ALIGNMENT, CLOSE, FORMAT, OPEN};
    let mut out = String::new();
    for p in parts {
        match p {
            StrPart::Text(t) => {
                for c in t.chars() {
                    out.push(c);
                    if c == OPEN || c == CLOSE {
                        out.push(c);
                    }
                }
            }
            StrPart::Expr {
                tokens,
                alignment,
                format,
            } => {
                out.push(OPEN);
                out.push_str(written(tokens, text));
                if let Some(a) = alignment {
                    out.push(ALIGNMENT);
                    out.push_str(a);
                }
                if let Some(f) = format {
                    out.push(FORMAT);
                    out.push_str(f);
                }
                out.push(CLOSE);
            }
        }
    }
    out
}
