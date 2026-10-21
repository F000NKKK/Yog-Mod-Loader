//! A `.yres` text's `{name}` / `{0}` placeholders: the accessor's
//! parameters, and Minecraft's `%1$s` in the lang files.

use std::fmt::Write as _;

use super::names::accessor_name;
use crate::rust_code::{is_ident, is_keyword};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Segment {
    Text(String),
    /// `{name}` or `{0}`.
    Arg(String),
}

/// Splits a text into literal parts and `{arg}` placeholders. Yog rich
/// text (`{b|…}`, `{#rrggbb|…}`: an opening with a `|` before any brace)
/// and its `\` escapes stay in the text as they are.
pub(super) fn segments(text: &str) -> Result<Vec<Segment>, String> {
    let mut out = Vec::new();
    let mut lit = String::new();
    let mut spans = 0usize;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if {
                let rest: String = chars
                    .clone()
                    .take_while(|&x| x != '{' && x != '}')
                    .collect();
                rest.contains('|')
            } =>
            {
                lit.push('{');
                spans += 1;
            }
            '}' if spans > 0 => {
                lit.push('}');
                spans -= 1;
            }
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                lit.push_str("{{");
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                lit.push_str("}}");
            }
            '{' => {
                let mut arg = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) => arg.push(c),
                        None => {
                            return Err("`{` without `}` (write `{{` for a literal brace)".into());
                        }
                    }
                }
                let arg = arg.trim().to_string();
                let numeric = !arg.is_empty() && arg.bytes().all(|b| b.is_ascii_digit());
                if !numeric && (!is_ident(&arg) || is_keyword(&arg)) {
                    return Err(format!(
                        "placeholder `{{{arg}}}` must be a number or an identifier (write `{{{{` for a literal brace)"
                    ));
                }
                if !lit.is_empty() {
                    out.push(Segment::Text(std::mem::take(&mut lit)));
                }
                out.push(Segment::Arg(arg));
            }
            '}' => return Err("`}` without `{` (write `}}` for a literal brace)".into()),
            c => lit.push(c),
        }
    }
    if !lit.is_empty() {
        out.push(Segment::Text(lit));
    }
    Ok(out)
}

/// The parameters of a neutral text, in order: `{0}`, `{1}`, … by number
/// (no gaps), named ones by first appearance; one style per text.
pub(super) fn params(segs: &[Segment]) -> Result<Vec<String>, String> {
    let args: Vec<&String> = segs
        .iter()
        .filter_map(|s| match s {
            Segment::Arg(a) => Some(a),
            Segment::Text(_) => None,
        })
        .collect();
    let numeric = args
        .iter()
        .filter(|a| a.bytes().all(|b| b.is_ascii_digit()))
        .count();
    if numeric > 0 && numeric < args.len() {
        return Err("mixes numbered `{0}` and named `{name}` placeholders — use one style".into());
    }
    let mut out: Vec<String> = Vec::new();
    if numeric > 0 {
        let mut nums: Vec<usize> = args.iter().filter_map(|a| a.parse().ok()).collect();
        nums.sort_unstable();
        nums.dedup();
        for (i, n) in nums.iter().enumerate() {
            if *n != i {
                return Err(format!(
                    "placeholder {{{i}}} is missing (numbered placeholders start at {{0}}, no gaps)"
                ));
            }
            out.push(n.to_string());
        }
    } else {
        for a in args {
            if !out.contains(a) {
                out.push(a.clone());
            }
        }
    }
    Ok(out)
}

/// Minecraft's form of a text: `%` → `%%`, the placeholder for parameter
/// `i` → `%{i+1}$s`. Errors on a placeholder not in `params`.
pub(super) fn lang_text(segs: &[Segment], params: &[String]) -> Result<String, String> {
    let mut out = String::new();
    for s in segs {
        match s {
            Segment::Text(t) => out.push_str(&t.replace('%', "%%")),
            Segment::Arg(a) => {
                let pos = params
                    .iter()
                    .position(|p| p == a)
                    .ok_or_else(|| format!("placeholder `{{{a}}}` isn't in the neutral text"))?;
                let _ = write!(out, "%{}$s", pos + 1);
            }
        }
    }
    Ok(out)
}

/// A parameter's name in the accessor: `{0}` → `arg0`, `{count}` → `count`.
pub(super) fn param_ident(p: &str) -> String {
    if p.bytes().all(|b| b.is_ascii_digit()) {
        format!("arg{p}")
    } else {
        accessor_name(p)
    }
}
