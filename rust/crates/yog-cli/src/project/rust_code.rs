//! Names in the Rust code the build generates: identifiers, keywords.

/// Rust's keywords, strict and reserved: not an identifier as they are.
const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while", "abstract", "become", "box", "do", "final", "gen", "macro", "override",
    "priv", "try", "typeof", "unsized", "virtual", "yield",
];

/// Whether `s` is one of Rust's keywords.
pub(crate) fn is_keyword(s: &str) -> bool {
    KEYWORDS.contains(&s)
}

/// Whether `s` is one identifier's text: a letter or `_`, then letters,
/// digits and `_`s (a keyword too).
pub(crate) fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `v` as an `f32` literal: `1.0`, `0.5`.
pub(crate) fn f32_lit(v: f32) -> String {
    format!("{v:?}")
}

/// What marks a name to fill in a template of generated code: `$NAME$`.
const FILL_MARK: char = '$';

/// `template` with each `$NAME$` replaced by its value — in one pass, so
/// a value is never filled itself.
pub(crate) fn fill(template: &str, values: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find(FILL_MARK) {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let value = after.find(FILL_MARK).and_then(|end| {
            values
                .iter()
                .find(|(k, _)| *k == &after[..end])
                .map(|(_, v)| (end, v))
        });
        match value {
            Some((end, v)) => {
                out.push_str(v);
                rest = &after[end + 1..];
            }
            None => {
                out.push(FILL_MARK);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}
