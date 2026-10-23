//! String literals each language reads back as the same text.

use crate::consts::{FIRST_PRINTABLE, LAST_ASCII};

/// A Rust string literal.
pub fn rust_str(text: &str) -> String {
    format!("{text:?}")
}

/// A Java string literal: control and non-ASCII characters as `\\uXXXX`
/// (UTF-16 units), so the source reads the same in any encoding.
pub fn java_str(text: &str) -> String {
    quoted(text)
}

/// A TypeScript string literal: Java's escapes, which JSON reads too.
pub fn ts_str(text: &str) -> String {
    quoted(text)
}

/// `c` as its UTF-16 units' `\\uXXXX` escapes.
fn utf16_escaped(c: char, out: &mut String) {
    let mut units = [0u16; 2];
    for unit in c.encode_utf16(&mut units) {
        out.push_str(&format!("\\u{unit:04x}"));
    }
}

/// `text` in double quotes: quotes, backslashes and the usual whitespace
/// escaped, anything unprintable or past ASCII as UTF-16 escapes.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (FIRST_PRINTABLE..=LAST_ASCII).contains(&c) => out.push(c),
            c => utf16_escaped(c, &mut out),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests;
