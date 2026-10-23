//! Quoting text as a literal, by a language's own escapes.

/// `text` between `delimiter`s, each character as `escape` says (`None`:
/// as itself).
pub fn quoted(text: &str, delimiter: char, escape: impl Fn(char) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push(delimiter);
    for c in text.chars() {
        match escape(c) {
            Some(escaped) => out.push_str(&escaped),
            None => out.push(c),
        }
    }
    out.push(delimiter);
    out
}

/// `c` as its UTF-16 units, each `prefix` and four hex digits (`é`).
pub fn utf16_units(c: char, prefix: &str) -> String {
    let mut units = [0u16; 2];
    c.encode_utf16(&mut units)
        .iter()
        .map(|unit| format!("{prefix}{unit:04x}"))
        .collect()
}

#[cfg(test)]
mod tests;
