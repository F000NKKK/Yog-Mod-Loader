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
