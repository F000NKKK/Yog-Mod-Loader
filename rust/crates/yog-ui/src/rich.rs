//! Yog text — one format for every text a mod writes (`.yres`, `.yui`,
//! books), the same for every host, version and the designer:
//!
//! | Write | Means |
//! |---|---|
//! | `{b\|bold}` `{i\|…}` `{u\|…}` `{s\|…}` | bold, italic, underlined, struck through |
//! | `{#8d6acc\|text}` | any color |
//! | `{b #8d6acc\|text}` | several at once; spans nest |
//! | `{name}`, `{0}` | an argument (`.yres` texts; filled in before drawing) |
//! | `{{`, `}}` | a literal `{`, `}` (`}}` outside a span; in one, `}` closes it) |
//!
//! A producer may add its own attribute words and turn them into these
//! before drawing (a book's `accent`, `link=entry` — see [`map_attrs`]).
//! Minecraft's own `§` codes pass through as the game reads them.

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

/// Whether `s` has anything but plain characters: span braces or a `§`
/// code (most words don't, and skip the tokenizer).
fn has_markup(s: &str) -> bool {
    // `§` is U+00A7, encoded C2 A7.
    s.bytes().any(|b| matches!(b, b'{' | b'}' | 0xC2))
}

/// One step of reading rich text.
enum Token<'a> {
    Char(char),
    /// A span opens: its attribute text.
    Open(&'a str),
    Close,
}

/// Reads `s` token by token.
fn tokens(s: &str) -> impl Iterator<Item = Token<'_>> {
    pieces(s).map(|(t, _)| t)
}

/// `s` read token by token, each with the text it was read from — lazily,
/// so measuring a word allocates nothing.
struct Pieces<'a> {
    s: &'a str,
    i: usize,
    depth: usize,
}

impl<'a> Iterator for Pieces<'a> {
    type Item = (Token<'a>, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let (s, i) = (self.s, self.i);
        let b = s.as_bytes();
        let c = s[i..].chars().next()?;
        match c {
            '{' if b.get(i + 1) == Some(&b'{') => {
                self.i += 2;
                return Some((Token::Char('{'), &s[i..i + 2]));
            }
            '{' => {
                if let Some(bar) = s[i + 1..].find(['|', '{', '}']).map(|p| p + i + 1)
                    && b[bar] == b'|'
                    && apply(&s[i + 1..bar], Style::default()).is_some()
                {
                    self.depth += 1;
                    self.i = bar + 1;
                    return Some((Token::Open(&s[i + 1..bar]), &s[i..=bar]));
                }
            }
            '}' if self.depth > 0 => {
                self.depth -= 1;
                self.i += 1;
                return Some((Token::Close, &s[i..=i]));
            }
            '}' if b.get(i + 1) == Some(&b'}') => {
                self.i += 2;
                return Some((Token::Char('}'), &s[i..i + 2]));
            }
            _ => {}
        }
        self.i += c.len_utf8();
        Some((Token::Char(c), &s[i..i + c.len_utf8()]))
    }
}

fn pieces(s: &str) -> Pieces<'_> {
    Pieces { s, i: 0, depth: 0 }
}

/// `s` split at its shown spaces (a span's `{b #fff|` stays whole).
pub fn words(s: &str) -> Vec<&str> {
    // Byte by byte, as `Pieces` reads it: the bytes that matter (space,
    // braces, `|`) are ASCII, never part of another character's encoding.
    let b = s.as_bytes();
    let mut out = Vec::new();
    let (mut start, mut i, mut depth) = (0, 0, 0usize);
    while i < b.len() {
        match b[i] {
            b'{' if b.get(i + 1) == Some(&b'{') => i += 2,
            b'{' => match s[i + 1..].find(['|', '{', '}']).map(|p| p + i + 1) {
                Some(bar) if b[bar] == b'|' && apply(&s[i + 1..bar], Style::default()).is_some() => {
                    depth += 1;
                    i = bar + 1;
                }
                _ => i += 1,
            },
            b'}' if depth > 0 => {
                depth -= 1;
                i += 1;
            }
            b'}' if b.get(i + 1) == Some(&b'}') => i += 2,
            b' ' => {
                out.push(&s[start..i]);
                start = i + 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    out.push(&s[start..]);
    out
}

/// `s` split into its markup and its characters: each piece's text as
/// written, and the character it shows (`None`: markup, no room taken).
pub fn glyphs(s: &str) -> Vec<(&str, Option<char>)> {
    pieces(s)
        .map(|(t, raw)| match t {
            Token::Char(c) => (raw, Some(c)),
            _ => (raw, None),
        })
        .collect()
}

/// `s` with every span's attribute words passed through `f` (`Some`: the
/// words to write instead) — how a producer turns its own words (`accent`,
/// `link=…`) into this format's.
pub fn map_attrs(s: &str, f: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        if b[i] == b'{' && b.get(i + 1) == Some(&b'{') {
            out.push_str("{{");
            i += 2;
            continue;
        }
        if b[i] == b'{'
            && let Some(bar) = s[i + 1..].find(['|', '{', '}']).map(|p| p + i + 1)
            && b[bar] == b'|'
        {
            let words: Vec<String> = s[i + 1..bar].split_whitespace().map(|w| f(w).unwrap_or_else(|| w.to_string())).collect();
            out.push('{');
            out.push_str(&words.join(" "));
            out.push('|');
            i = bar + 1;
            continue;
        }
        let c = s[i..].chars().next().unwrap_or_default();
        out.push(c);
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
    if !has_markup(s) {
        return s.chars().map(advance).sum();
    }
    // As `runs` reads it, without building the runs: a `§` code acts on
    // the glyph after it within one stretch of the same style.
    let mut w = 0.0;
    let mut outer: Vec<Style> = Vec::new();
    let mut style = Style::default();
    let mut legacy_bold = false;
    let mut code = false;
    for t in tokens(s) {
        match t {
            Token::Char(c) => {
                if code {
                    code = false;
                    match c.to_ascii_lowercase() {
                        'l' => legacy_bold = true,
                        k if k == 'r' || k.is_ascii_hexdigit() => legacy_bold = false,
                        _ => {}
                    }
                    continue;
                }
                if c == '§' {
                    code = true;
                    continue;
                }
                w += advance(c) + if style.bold || legacy_bold { 1.0 } else { 0.0 };
            }
            Token::Open(a) => {
                outer.push(style);
                let next = apply(a, style).unwrap_or(style);
                if next != style {
                    (legacy_bold, code) = (false, false);
                }
                style = next;
            }
            Token::Close => {
                if let Some(prev) = outer.pop() {
                    if prev != style {
                        (legacy_bold, code) = (false, false);
                    }
                    style = prev;
                }
            }
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
    /// `width` as it was computed before it stopped allocating: through `runs`.
    fn width_via_runs(s: &str, advance: impl Fn(char) -> f32) -> f32 {
        let mut w = 0.0;
        for (text, st) in super::runs(s) {
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

    /// `words` as it was: splitting at the space tokens of `pieces`.
    fn words_via_pieces(s: &str) -> Vec<&str> {
        let (mut out, mut start, mut at) = (Vec::new(), 0, 0);
        for (t, raw) in super::pieces(s) {
            if matches!(t, super::Token::Char(' ')) {
                out.push(&s[start..at]);
                start = at + 1;
            }
            at += raw.len();
        }
        out.push(&s[start..]);
        out
    }

    #[test]
    fn words_split_as_pieces_do() {
        for s in ["a b  c", "{b #ff0000|bold words} x", "{{a b}} c", "{nope b|x} y", "ünï ç {i|é ö}", "{b|unclosed a b", " lead trail "] {
            assert_eq!(super::words(s), words_via_pieces(s), "{s:?}");
        }
    }

    #[test]
    fn width_measures_as_runs_do() {
        let adv = |c: char| if c == 'i' { 2.0 } else { 6.0 };
        for s in ["plain", "{b|bold} x", "{b|a {#ff0000|b} c}d", "§lbold§r then", "{{x}} }}", "{nope|x}", "{b|unclosed", "a§", "{b|§lx} y", "ünï {i|ç}"] {
            assert_eq!(super::width(s, adv), width_via_runs(s, adv), "{s:?}");
        }
    }

    use super::*;

    #[test]
    fn parses_spans() {
        let r = runs("a {b|bold {#8d6acc|purple}} {{x}} {not a span}");
        assert_eq!(r[0], ("a ".to_string(), Style::default()));
        assert_eq!(r[1].0, "bold ");
        assert!(r[1].1.bold);
        assert_eq!(r[2], ("purple".to_string(), Style { bold: true, color: Some(0x8d6acc), ..Default::default() }));
        assert_eq!(r[3].0, " {x} {not a span}");
        assert_eq!(plain("{i|x}y"), "xy");
    }

    #[test]
    fn producers_map_their_words() {
        let s = map_attrs("a {accent|term} {{x}} {b link=m:e|y}", |w| match w {
            "accent" => Some("#8d6acc".into()),
            w if w.starts_with("link=") => Some("u #2f5fbf".into()),
            _ => None,
        });
        assert_eq!(s, "a {#8d6acc|term} {{x}} {b u #2f5fbf|y}");
    }

    #[test]
    fn spans_carry_over_lines() {
        assert_eq!(open_spans("a {b|b {#ffffff|c} d"), ["b"]);
        assert_eq!(reopen(&["b".into(), "#123456".into()]), "{b|{#123456|");
        assert_eq!(width("{b|ab}", |_| 5.0), 12.0, "bold is one wider a glyph");
        assert_eq!(width("§5ab", |_| 5.0), 10.0);
    }
}
