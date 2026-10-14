//! The text syntax: JSON, plus `//` and `/* */` comments, trailing commas
//! and unquoted keys (the part of JSON5 editors already understand).
//!
//! [`parse`] keeps where every value is ([`Spans`]), so a schema error at
//! `entries[2].pages[0].item` can point at its line. Typed values stay
//! strings here — the schema decides what a string means.

use std::fmt;

use crate::value::{Map, Path, Seg, Value};

/// Deeper than this is an error, not a stack overflow.
pub const MAX_DEPTH: usize = 256;

/// A byte range of the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

/// Where each value of a parsed document is: the value's span, its key's
/// (map entries), and its children's, in the same order as the value's.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Spans {
    pub span: Span,
    pub key: Option<Span>,
    pub children: Vec<Spans>,
}

/// A parsed document: the value and where each of its parts is.
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    pub value: Value,
    pub spans: Spans,
}

impl Doc {
    /// The spans of the value at `path` — the closest existing parent's if
    /// the path goes further than the document.
    pub fn spans_at(&self, path: &Path) -> &Spans {
        let mut v = &self.value;
        let mut s = &self.spans;
        for seg in path.segments() {
            let i = match (seg, v) {
                (Seg::Key(k), Value::Map(m)) => m.index_of(k),
                (Seg::Index(i), Value::List(l)) if *i < l.len() => Some(*i),
                _ => None,
            };
            let Some(i) = i else { break };
            let Some(child) = s.children.get(i) else {
                break;
            };
            v = match v {
                Value::Map(m) => m.value_at(i).unwrap_or(v),
                Value::List(l) => &l[i],
                _ => v,
            };
            s = child;
        }
        s
    }

    /// The line and column of the value at `path` in `text` (the source this was parsed from).
    pub fn locate(&self, text: &str, path: &Path) -> LineCol {
        LineCol::of(text, self.spans_at(path).span.start as usize)
    }
}

/// A 1-based line and column (in characters).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineCol {
    pub line: u32,
    pub col: u32,
}

impl LineCol {
    /// Where byte `offset` of `text` is.
    pub fn of(text: &str, offset: usize) -> LineCol {
        let offset = offset.min(text.len());
        let before = &text.as_bytes()[..offset];
        let line = before.iter().filter(|&&b| b == b'\n').count() as u32 + 1;
        let line_start = before
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |i| i + 1);
        let col = text
            .get(line_start..offset)
            .map_or(offset - line_start, |s| s.chars().count()) as u32
            + 1;
        LineCol { line, col }
    }
}

impl fmt::Display for LineCol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// A syntax error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub message: String,
    pub at: LineCol,
    pub offset: usize,
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.at, self.message)
    }
}

impl std::error::Error for SyntaxError {}

/// Parses Yog Data text.
pub fn parse(text: &str) -> Result<Doc, SyntaxError> {
    let (value, spans) = run::<true>(text)?;
    Ok(Doc { value, spans })
}

/// Parses Yog Data text, without the positions (faster).
pub fn parse_value(text: &str) -> Result<Value, SyntaxError> {
    run::<false>(text).map(|(v, _)| v)
}

fn run<const SPANS: bool>(text: &str) -> Result<(Value, Spans), SyntaxError> {
    let mut p = Parser::<SPANS> {
        src: text.as_bytes(),
        text,
        i: 0,
        depth: 0,
    };
    if text.starts_with('\u{feff}') {
        p.i = 3;
    }
    p.ws()?;
    let (value, spans) = p.value()?;
    p.ws()?;
    if p.i < p.src.len() {
        return Err(p.error("unexpected text after the value"));
    }
    Ok((value, spans))
}

/// `SPANS`: whether to record where each value is.
struct Parser<'a, const SPANS: bool> {
    src: &'a [u8],
    text: &'a str,
    i: usize,
    depth: usize,
}

impl<const SPANS: bool> Parser<'_, SPANS> {
    fn error(&self, message: impl Into<String>) -> SyntaxError {
        self.error_at(self.i, message)
    }

    fn error_at(&self, offset: usize, message: impl Into<String>) -> SyntaxError {
        let mut message = message.into();
        if offset >= self.src.len() {
            message.push_str(", found the end of the file");
        }
        SyntaxError {
            message,
            at: LineCol::of(self.text, offset),
            offset,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.i).copied()
    }

    /// Skips whitespace and comments.
    fn ws(&mut self) -> Result<(), SyntaxError> {
        loop {
            let rest = &self.src[self.i..];
            self.i += rest
                .iter()
                .position(|&b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
                .unwrap_or(rest.len());
            if self.peek() != Some(b'/') {
                return Ok(());
            }
            match self.src.get(self.i + 1) {
                Some(b'/') => {
                    let rest = &self.src[self.i..];
                    self.i += rest.iter().position(|&b| b == b'\n').unwrap_or(rest.len());
                }
                Some(b'*') => {
                    let start = self.i;
                    match self.text[self.i + 2..].find("*/") {
                        Some(end) => self.i += 2 + end + 2,
                        None => return Err(self.error_at(start, "unclosed `/*` comment")),
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn value(&mut self) -> Result<(Value, Spans), SyntaxError> {
        let start = self.i;
        let (value, children) = match self.peek() {
            Some(b'{') => self.nested(|p| p.map())?,
            Some(b'[') => self.nested(|p| p.list())?,
            Some(b'"') => (Value::Str(self.string()?), Vec::new()),
            Some(b'-' | b'0'..=b'9') => (self.number()?, Vec::new()),
            Some(b't') if self.src[self.i..].starts_with(b"true") => {
                self.i += 4;
                (Value::Bool(true), Vec::new())
            }
            Some(b'f') if self.src[self.i..].starts_with(b"false") => {
                self.i += 5;
                (Value::Bool(false), Vec::new())
            }
            Some(b'n') if self.src[self.i..].starts_with(b"null") => {
                self.i += 4;
                (Value::Null, Vec::new())
            }
            Some(b'\'') => return Err(self.error("strings use double quotes")),
            _ => return Err(self.error("expected a value")),
        };
        let span = Span {
            start: start as u32,
            end: self.i as u32,
        };
        Ok((
            value,
            Spans {
                span,
                key: None,
                children,
            },
        ))
    }

    fn nested(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<(Value, Vec<Spans>), SyntaxError>,
    ) -> Result<(Value, Vec<Spans>), SyntaxError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.error(format!("nested deeper than {MAX_DEPTH} levels")));
        }
        let r = f(self);
        self.depth -= 1;
        r
    }

    fn list(&mut self) -> Result<(Value, Vec<Spans>), SyntaxError> {
        self.i += 1;
        let mut items = Vec::new();
        let mut spans = Vec::new();
        loop {
            self.ws()?;
            if self.peek() == Some(b']') {
                self.i += 1;
                break;
            }
            let (v, s) = self.value()?;
            items.push(v);
            if SPANS {
                spans.push(s);
            }
            self.ws()?;
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(self.error("expected `,` or `]`")),
            }
        }
        Ok((Value::List(items), spans))
    }

    fn map(&mut self) -> Result<(Value, Vec<Spans>), SyntaxError> {
        self.i += 1;
        let mut map = Map::new();
        let mut spans = Vec::new();
        // Duplicate keys: a scan for small objects, a set once they grow.
        let mut seen: Option<std::collections::HashSet<String>> = None;
        loop {
            self.ws()?;
            if self.peek() == Some(b'}') {
                self.i += 1;
                break;
            }
            let key_start = self.i;
            let key = match self.peek() {
                Some(b'"') => self.string()?,
                Some(c) if c.is_ascii_alphabetic() || c == b'_' || c == b'$' => {
                    while self
                        .peek()
                        .is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'$')
                    {
                        self.i += 1;
                    }
                    self.text[key_start..self.i].to_string()
                }
                _ => return Err(self.error("expected a key")),
            };
            let key_span = Span {
                start: key_start as u32,
                end: self.i as u32,
            };
            let duplicate = match &mut seen {
                Some(set) => !set.insert(key.clone()),
                None if map.len() < 16 => map.get(&key).is_some(),
                None => {
                    let mut set: std::collections::HashSet<String> =
                        map.keys().map(str::to_string).collect();
                    let dup = !set.insert(key.clone());
                    seen = Some(set);
                    dup
                }
            };
            if duplicate {
                return Err(
                    self.error_at(key_start, format!("`{key}` is already set in this object"))
                );
            }
            self.ws()?;
            if self.peek() != Some(b':') {
                return Err(self.error("expected `:` after the key"));
            }
            self.i += 1;
            self.ws()?;
            let (v, mut s) = self.value()?;
            s.key = Some(key_span);
            map.push_unchecked(key.into(), v);
            if SPANS {
                spans.push(s);
            }
            self.ws()?;
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(self.error("expected `,` or `}`")),
            }
        }
        Ok((Value::Map(map), spans))
    }

    fn string(&mut self) -> Result<String, SyntaxError> {
        let open = self.i;
        self.i += 1;
        let mut out = String::new();
        loop {
            let rest = &self.src[self.i..];
            let Some(n) = rest
                .iter()
                .position(|&b| b == b'"' || b == b'\\' || b < 0x20)
            else {
                return Err(self.error_at(open, "unclosed string"));
            };
            out.push_str(&self.text[self.i..self.i + n]);
            self.i += n;
            match self.src[self.i] {
                b'"' => {
                    self.i += 1;
                    return Ok(out);
                }
                b'\\' => self.escape(&mut out)?,
                b'\n' => return Err(self.error("line break in a string (write `\\n`)")),
                _ => return Err(self.error("control character in a string")),
            }
        }
    }

    fn escape(&mut self, out: &mut String) -> Result<(), SyntaxError> {
        let at = self.i;
        self.i += 1;
        let c = self.peek().ok_or_else(|| self.error("unclosed string"))?;
        self.i += 1;
        match c {
            b'"' => out.push('"'),
            b'\\' => out.push('\\'),
            b'/' => out.push('/'),
            b'b' => out.push('\u{8}'),
            b'f' => out.push('\u{c}'),
            b'n' => out.push('\n'),
            b'r' => out.push('\r'),
            b't' => out.push('\t'),
            b'u' => {
                let hi = self.hex4(at)?;
                let ch = if (0xD800..0xDC00).contains(&hi) {
                    if !self.src[self.i..].starts_with(b"\\u") {
                        return Err(self.error_at(at, "unpaired surrogate in `\\u` escape"));
                    }
                    self.i += 2;
                    let lo = self.hex4(at)?;
                    if !(0xDC00..0xE000).contains(&lo) {
                        return Err(self.error_at(at, "unpaired surrogate in `\\u` escape"));
                    }
                    char::from_u32(0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00))
                } else {
                    char::from_u32(hi)
                };
                out.push(
                    ch.ok_or_else(|| self.error_at(at, "unpaired surrogate in `\\u` escape"))?,
                );
            }
            _ => {
                return Err(self.error_at(
                    at,
                    "unknown escape (`\\n`, `\\t`, `\\\"`, `\\\\`, `\\uXXXX`, ...)",
                ));
            }
        }
        Ok(())
    }

    fn hex4(&mut self, at: usize) -> Result<u32, SyntaxError> {
        let digits = self
            .text
            .get(self.i..self.i + 4)
            .filter(|d| d.bytes().all(|b| b.is_ascii_hexdigit()));
        let d = digits.ok_or_else(|| self.error_at(at, "`\\u` needs four hex digits"))?;
        self.i += 4;
        Ok(u32::from_str_radix(d, 16).unwrap())
    }

    fn number(&mut self) -> Result<Value, SyntaxError> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        let digits = |p: &mut Self| {
            let s = p.i;
            while p.peek().is_some_and(|c| c.is_ascii_digit()) {
                p.i += 1;
            }
            p.i - s
        };
        let int_start = self.i;
        if digits(self) == 0 {
            return Err(self.error("expected digits"));
        }
        if self.src[int_start] == b'0' && self.i - int_start > 1 {
            return Err(self.error_at(int_start, "numbers don't start with 0"));
        }
        let mut float = false;
        if self.peek() == Some(b'.') {
            self.i += 1;
            float = true;
            if digits(self) == 0 {
                return Err(self.error("expected digits after `.`"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            float = true;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if digits(self) == 0 {
                return Err(self.error("expected digits in the exponent"));
            }
        }
        let s = &self.text[start..self.i];
        if !float && let Ok(n) = s.parse::<i64>() {
            return Ok(Value::Int(n));
        }
        Ok(Value::Float(
            s.parse::<f64>()
                .map_err(|_| self.error_at(start, "invalid number"))?,
        ))
    }
}

// ── Writing ─────────────────────────────────────────────────────────────────

/// `value` as compact text (typed values in their string forms).
pub fn to_string(value: &Value) -> String {
    let mut out = String::with_capacity(256);
    write(value, None, 0, &mut out);
    out
}

/// `value` as text indented by two spaces, one entry per line.
pub fn to_string_pretty(value: &Value) -> String {
    let mut out = String::new();
    write(value, Some("  "), 0, &mut out);
    out.push('\n');
    out
}

fn write(value: &Value, indent: Option<&str>, level: usize, out: &mut String) {
    let newline = |out: &mut String, level: usize| {
        if let Some(unit) = indent {
            out.push('\n');
            for _ in 0..level {
                out.push_str(unit);
            }
        }
    };
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(n) => {
            use std::fmt::Write as _;
            let _ = write!(out, "{n}");
        }
        Value::Float(f) => write_float(*f, out),
        Value::List(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, v) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, level + 1);
                write(v, indent, level + 1, out);
            }
            newline(out, level);
            out.push(']');
        }
        Value::Map(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, v)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, level + 1);
                write_string(k, out);
                out.push_str(if indent.is_some() { ": " } else { ":" });
                write(v, indent, level + 1, out);
            }
            newline(out, level);
            out.push('}');
        }
        Value::Str(s) | Value::Text(s) => {
            if s.starts_with('@') {
                out.push('"');
                out.push('@');
                write_string_body(s, out);
                out.push('"');
            } else {
                write_string(s, out);
            }
        }
        typed => write_string(&crate::typed::typed_string(typed).unwrap_or_default(), out),
    }
}

fn write_float(f: f64, out: &mut String) {
    if !f.is_finite() {
        out.push_str("null");
    } else if f.fract() == 0.0 && f.abs() < 1e16 {
        out.push_str(&format!("{f:.1}"));
    } else {
        out.push_str(&f.to_string());
    }
}

/// `s` as a JSON string literal.
pub fn write_string(s: &str, out: &mut String) {
    out.push('"');
    write_string_body(s, out);
    out.push('"');
}

fn write_string_body(s: &str, out: &mut String) {
    let bytes = s.as_bytes();
    let mut run = 0;
    while let Some(n) = bytes[run..]
        .iter()
        .position(|&b| b == b'"' || b == b'\\' || b < 0x20)
    {
        let i = run + n;
        out.push_str(&s[run..i]);
        match bytes[i] {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            b => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{b:04x}");
            }
        }
        run = i + 1;
    }
    out.push_str(&s[run..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json5_subset() {
        let doc = parse("\u{feff}{\n  // comment\n  name: \"Ruby\", /* inline */ $schema: 1,\n  \"list\": [1, 2.5, -3e2, true, null,],\n}").unwrap();
        let m = doc.value.as_map().unwrap();
        assert_eq!(m.keys().collect::<Vec<_>>(), ["name", "$schema", "list"]);
        assert_eq!(
            doc.value.get("list").unwrap(),
            &Value::List(vec![
                Value::Int(1),
                Value::Float(2.5),
                Value::Float(-300.0),
                Value::Bool(true),
                Value::Null
            ])
        );
    }

    #[test]
    fn locates_values_and_keys() {
        let text = "{\n  \"a\": [\n    1,\n    {\"b\": \"x\"}\n  ]\n}";
        let doc = parse(text).unwrap();
        let path = Path::root().key("a").index(1).key("b");
        assert_eq!(doc.locate(text, &path), LineCol { line: 4, col: 11 });
        assert_eq!(
            LineCol::of(text, doc.spans_at(&path).key.unwrap().start as usize),
            LineCol { line: 4, col: 6 }
        );
        // A path past the document: its deepest existing part.
        assert_eq!(
            doc.locate(text, &Path::root().key("a").index(9)),
            LineCol { line: 2, col: 8 }
        );
    }

    #[test]
    fn errors_say_where() {
        let cases = [
            ("{\"a\": 1,, }", "1:9", "expected a key"),
            ("{\"a\": 1 \"b\": 2}", "1:9", "expected `,` or `}`"),
            ("{\"a\": 1, \"a\": 2}", "1:10", "already set"),
            ("[1, 2", "1:6", "found the end of the file"),
            ("\"ab\ncd\"", "1:4", "line break"),
            ("'x'", "1:1", "double quotes"),
            ("01", "1:1", "start with 0"),
            ("{} x", "1:4", "unexpected text"),
            ("/* never closed", "1:1", "unclosed"),
            ("\"\\q\"", "1:2", "unknown escape"),
            ("\"\\ud800\"", "1:2", "surrogate"),
            ("{\n  \"é\": tru }", "2:8", "expected a value"),
        ];
        for (text, at, msg) in cases {
            let e = parse(text).unwrap_err();
            assert_eq!(e.at.to_string(), at, "{text:?}: {e}");
            assert!(e.message.contains(msg), "{text:?}: {e}");
        }
        assert!(
            parse(&"[".repeat(MAX_DEPTH + 1))
                .unwrap_err()
                .message
                .contains("nested deeper")
        );
    }

    #[test]
    fn strings_and_numbers() {
        let v = parse_value(
            r#"["a\"b\\c\n\u00e9\ud83d\ude00", 9223372036854775807, 9223372036854775808, 0.1]"#,
        )
        .unwrap();
        let l = v.as_list().unwrap();
        assert_eq!(l[0], Value::Str("a\"b\\c\né😀".into()));
        assert_eq!(l[1], Value::Int(i64::MAX));
        assert_eq!(l[2], Value::Float(9223372036854775808.0));
        assert_eq!(l[3], Value::Float(0.1));
    }

    #[test]
    fn writes_what_it_reads() {
        let text = "{\n  \"name\": \"Ruby \\\"red\\\"\\n\",\n  \"n\": [1, 2.0, 0.1, -5],\n  \"empty\": {},\n  \"none\": [],\n  \"tab\\u0001\": null\n}\n";
        let v = parse_value(text).unwrap();
        assert_eq!(parse_value(&to_string(&v)).unwrap(), v);
        let pretty = to_string_pretty(&v);
        assert_eq!(parse_value(&pretty).unwrap(), v);
        assert!(
            pretty.starts_with(
                "{\n  \"name\": \"Ruby \\\"red\\\"\\n\",\n  \"n\": [\n    1,\n    2.0,"
            ),
            "{pretty}"
        );
        assert_eq!(
            to_string(&v),
            r#"{"name":"Ruby \"red\"\n","n":[1,2.0,0.1,-5],"empty":{},"none":[],"tab\u0001":null}"#
        );
    }

    #[test]
    fn typed_values_are_strings_in_text() {
        let v = Value::List(vec![
            Value::Ref(crate::Ref::new("item", "minecraft:stick")),
            Value::Color(0xFF112233),
            Value::Str("@not a ref".into()),
        ]);
        assert_eq!(
            to_string(&v),
            r##"["@item:minecraft:stick","#112233","@@not a ref"]"##
        );
    }
}
