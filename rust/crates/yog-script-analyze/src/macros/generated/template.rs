//! A message template (`"Moved {Count} items to {Target,-8:F1}"`) read at
//! build time: its text and holes, written back as Yog Script string
//! literals — the template itself, and the message the compiler formats.

use crate::lexer::string::{ALIGNMENT, CLOSE, ESCAPE, ESCAPES, FORMAT, OPEN, QUOTE};

/// A piece of a template.
pub(super) enum Piece {
    Text(String),
    Hole {
        name: String,
        /// `,alignment:format`, as written.
        rest: String,
    },
}

/// Template `t` in pieces (`{{` and `}}` are braces); `None`: a hole left open.
pub(super) fn pieces(t: &str) -> Option<Vec<Piece>> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        if (c == OPEN || c == CLOSE) && chars.peek() == Some(&c) {
            chars.next();
            text.push(c);
        } else if c == OPEN {
            let hole: String = chars.by_ref().take_while(|&h| h != CLOSE).collect();
            let split = hole.find([ALIGNMENT, FORMAT]).unwrap_or(hole.len());
            out.push(Piece::Text(std::mem::take(&mut text)));
            out.push(Piece::Hole {
                name: hole[..split].trim().to_string(),
                rest: hole[split..].to_string(),
            });
        } else {
            text.push(c);
        }
    }
    out.push(Piece::Text(text));
    let closed = t.matches(OPEN).count() == t.matches(CLOSE).count();
    closed.then_some(out)
}

/// The holes' names, each once, in order.
pub(super) fn names(pieces: &[Piece]) -> Vec<&str> {
    let mut names: Vec<&str> = Vec::new();
    for p in pieces {
        if let Piece::Hole { name, .. } = p
            && !names.iter().any(|n| n.eq_ignore_ascii_case(name))
        {
            names.push(name);
        }
    }
    names
}

/// The message as a Yog Script interpolated string's inside: each hole
/// the field `field(name)` names, its alignment and format kept.
pub(super) fn message(pieces: &[Piece], field: impl Fn(&str) -> String) -> String {
    let mut out = String::new();
    for p in pieces {
        match p {
            Piece::Text(t) => out.push_str(&escaped(t)),
            Piece::Hole { name, rest } => {
                out.push(OPEN);
                out.push_str(&field(name));
                out.push_str(rest);
                out.push(CLOSE);
            }
        }
    }
    out
}

/// Text `t` as a Yog Script string literal's inside, nothing in it a hole.
pub(super) fn escaped(t: &str) -> String {
    let mut out = String::new();
    for c in t.chars() {
        match ESCAPES.iter().find(|(_, v)| *v == c) {
            Some((k, _)) => {
                out.push(ESCAPE);
                out.push(*k);
            }
            None if [ESCAPE, QUOTE, OPEN, CLOSE].contains(&c) => {
                out.push(ESCAPE);
                out.push(c);
            }
            None => out.push(c),
        }
    }
    out
}
