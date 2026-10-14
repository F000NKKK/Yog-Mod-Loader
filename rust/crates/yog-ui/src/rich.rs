//! Yog rich text — formatting inside a text string, the same for every
//! host, version and the designer:
//!
//! | Write | Shows |
//! |---|---|
//! | `{b\|bold}` | bold |
//! | `{i\|italic}` | italic |
//! | `{u\|underlined}` | underlined |
//! | `{s\|struck}` | struck through |
//! | `{#8d6acc\|text}` | any color (`#rrggbb`) |
//! | `{b #8d6acc\|text}` | several at once; spans nest |
//! | `\{` `\}` `\|` `\\` | the character itself |
//!
//! A `{` not starting a span (no `|`, unknown attributes) is just a
//! character. Minecraft's own `§` codes pass through as the game reads them.

/// What a stretch of text looks like.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// `0xRRGGBB`; `None`: the text's own color.
    pub color: Option<u32>,
}

/// The attributes of a span opening `{attrs|`, applied to `style`;
/// `None` if they aren't attributes.
fn apply(attrs: &str, mut style: Style) -> Option<Style> {
    let mut any = false;
    for a in attrs.split_whitespace() {
        any = true;
        match a {
            "b" => style.bold = true,
            "i" => style.italic = true,
            "u" => style.underline = true,
            "s" => style.strike = true,
            hex if hex.len() == 7 && hex.starts_with('#') => style.color = Some(u32::from_str_radix(&hex[1..], 16).ok()?),
            _ => return None,
        }
    }
    any.then_some(style)
}

/// One step of reading rich text.
enum Token<'a> {
    Char(char),
    /// A span opens: its attribute text.
    Open(&'a str),
    Close,
}

/// Reads `s` token by token (a `}` with no span open is a character).
fn tokens(s: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    let b = s.as_bytes();
    while i < s.len() {
        let c = s[i..].chars().next().unwrap_or_default();
        match c {
            '\\' => {
                if let Some(n) = s[i + 1..].chars().next() {
                    out.push(Token::Char(n));
                    i += 1 + n.len_utf8();
                } else {
                    out.push(Token::Char('\\'));
                    i += 1;
                }
                continue;
            }
            '{' => {
                if let Some(bar) = s[i + 1..].find(['|', '{', '}']).map(|p| p + i + 1)
                    && b[bar] == b'|'
                    && apply(&s[i + 1..bar], Style::default()).is_some()
                {
                    out.push(Token::Open(&s[i + 1..bar]));
                    depth += 1;
                    i = bar + 1;
                    continue;
                }
            }
            '}' if depth > 0 => {
                out.push(Token::Close);
                depth -= 1;
                i += 1;
                continue;
            }
            _ => {}
        }
        out.push(Token::Char(c));
        i += c.len_utf8();
    }
    out
}

/// `s` as runs of one style each.
pub fn runs(s: &str) -> Vec<(String, Style)> {
    let mut out: Vec<(String, Style)> = Vec::new();
    let mut stack = vec![Style::default()];
    for t in tokens(s) {
        match t {
            Token::Char(c) => {
                let st = *stack.last().unwrap_or(&Style::default());
                match out.last_mut() {
                    Some((text, last)) if *last == st => text.push(c),
                    _ => out.push((c.to_string(), st)),
                }
            }
            Token::Open(a) => {
                let top = *stack.last().unwrap_or(&Style::default());
                stack.push(apply(a, top).unwrap_or(top));
            }
            Token::Close => {
                if stack.len() > 1 {
                    stack.pop();
                }
            }
        }
    }
    out
}

/// The text without its markup.
pub fn plain(s: &str) -> String {
    runs(s).into_iter().map(|(t, _)| t).collect()
}

/// Width with `advance` per glyph: markup takes no room, bold glyphs one
/// more (Minecraft's own `§` codes too).
pub fn width(s: &str, mut advance: impl FnMut(char) -> f32) -> f32 {
    let mut w = 0.0;
    for (text, st) in runs(s) {
        let mut legacy_bold = false;
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c == '§' {
                match chars.next().map(|k| k.to_ascii_lowercase()) {
                    Some('l') => legacy_bold = true,
                    Some(k) if k == 'r' || k.is_ascii_hexdigit() => legacy_bold = false,
                    _ => {}
                }
                continue;
            }
            w += advance(c) + if st.bold || legacy_bold { 1.0 } else { 0.0 };
        }
    }
    w
}

/// The spans still open at the end of `s`, outermost first (their
/// attribute text) — a wrapped line closes them and the next reopens them,
/// since every line is drawn on its own.
pub fn open_spans(s: &str) -> Vec<String> {
    let mut stack = Vec::new();
    for t in tokens(s) {
        match t {
            Token::Open(a) => stack.push(a.to_string()),
            Token::Close => {
                stack.pop();
            }
            Token::Char(_) => {}
        }
    }
    stack
}

/// `{a|` for each of `spans`.
pub fn reopen(spans: &[String]) -> String {
    spans.iter().map(|a| format!("{{{a}|")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spans() {
        let r = runs("a {b|bold {#8d6acc|purple}} \\{x\\} {not a span}");
        assert_eq!(r[0], ("a ".to_string(), Style::default()));
        assert_eq!(r[1].0, "bold ");
        assert!(r[1].1.bold);
        assert_eq!(r[2], ("purple".to_string(), Style { bold: true, color: Some(0x8d6acc), ..Default::default() }));
        assert_eq!(r[3].0, " {x} {not a span}");
        assert_eq!(plain("{i|x}y"), "xy");
    }

    #[test]
    fn spans_carry_over_lines() {
        assert_eq!(open_spans("a {b|b {#ffffff|c} d"), ["b"]);
        assert_eq!(reopen(&["b".into(), "#123456".into()]), "{b|{#123456|");
        assert_eq!(width("{b|ab}", |_| 5.0), 12.0, "bold is one wider a glyph");
        assert_eq!(width("§5ab", |_| 5.0), 10.0);
    }
}
