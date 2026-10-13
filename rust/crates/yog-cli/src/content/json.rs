//! A small JSON reader that remembers the line of every value, so `.yblock`
//! and `.yitem` errors point at `file:line` (serde_json only knows where a
//! *syntax* error is).

/// A JSON value and the 1-based line it starts on.
#[derive(Debug, Clone, PartialEq)]
pub struct J {
    pub line: usize,
    pub v: V,
}

#[derive(Debug, Clone, PartialEq)]
pub enum V {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    /// Members in file order.
    Obj(Vec<(String, J)>),
}

impl J {
    pub fn get(&self, key: &str) -> Option<&J> {
        match &self.v {
            V::Obj(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn members(&self) -> &[(String, J)] {
        match &self.v {
            V::Obj(m) => m,
            _ => &[],
        }
    }

    pub fn items(&self) -> &[J] {
        match &self.v {
            V::Arr(a) => a,
            _ => &[],
        }
    }

    pub fn str(&self) -> Option<&str> {
        match &self.v {
            V::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn num(&self) -> Option<f64> {
        match self.v {
            V::Num(n) => Some(n),
            _ => None,
        }
    }

    pub fn bool(&self) -> Option<bool> {
        match self.v {
            V::Bool(b) => Some(b),
            _ => None,
        }
    }

    pub fn is_obj(&self) -> bool {
        matches!(self.v, V::Obj(_))
    }

    pub fn is_arr(&self) -> bool {
        matches!(self.v, V::Arr(_))
    }

    /// What kind of value this is, for messages ("a string", "an object").
    pub fn kind(&self) -> &'static str {
        match self.v {
            V::Null => "null",
            V::Bool(_) => "a boolean",
            V::Num(_) => "a number",
            V::Str(_) => "a string",
            V::Arr(_) => "an array",
            V::Obj(_) => "an object",
        }
    }

    /// The plain serde_json form (lines dropped).
    #[cfg(test)]
    pub fn to_value(&self) -> serde_json::Value {
        use serde_json::Value;
        match &self.v {
            V::Null => Value::Null,
            V::Bool(b) => Value::Bool(*b),
            V::Num(n) => num_value(*n),
            V::Str(s) => Value::String(s.clone()),
            V::Arr(a) => Value::Array(a.iter().map(J::to_value).collect()),
            V::Obj(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), v.to_value())).collect()),
        }
    }
}

/// A number as JSON: integral values without a fraction (`16`, not `16.0`).
pub fn num_value(n: f64) -> serde_json::Value {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        serde_json::Value::from(n as i64)
    } else {
        serde_json::Number::from_f64(n)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null)
    }
}

/// Builds values for [`to_pretty`]: `J::obj([("a", J::number(1.0))])`.
impl J {
    pub fn of(v: V) -> J {
        J { line: 0, v }
    }

    pub fn obj<K: Into<String>>(members: impl IntoIterator<Item = (K, J)>) -> J {
        J::of(V::Obj(members.into_iter().map(|(k, v)| (k.into(), v)).collect()))
    }

    pub fn number(n: f64) -> J {
        J::of(V::Num(n))
    }

    pub fn string(s: impl Into<String>) -> J {
        J::of(V::Str(s.into()))
    }

    pub fn nums(ns: &[f64]) -> J {
        J::of(V::Arr(ns.iter().map(|n| J::number(*n)).collect()))
    }

    /// From serde_json (keys in its order).
    pub fn from_value(v: &serde_json::Value) -> J {
        use serde_json::Value;
        J::of(match v {
            Value::Null => V::Null,
            Value::Bool(b) => V::Bool(*b),
            Value::Number(n) => V::Num(n.as_f64().unwrap_or(0.0)),
            Value::String(s) => V::Str(s.clone()),
            Value::Array(a) => V::Arr(a.iter().map(J::from_value).collect()),
            Value::Object(m) => V::Obj(m.iter().map(|(k, v)| (k.clone(), J::from_value(v))).collect()),
        })
    }

    /// Adds or replaces a member of an object.
    pub fn set(&mut self, key: &str, value: J) {
        if let V::Obj(m) = &mut self.v {
            match m.iter_mut().find(|(k, _)| k == key) {
                Some((_, v)) => *v = value,
                None => m.push((key.to_string(), value)),
            }
        }
    }
}

/// Pretty JSON, keys in order, two-space indent; arrays of numbers and
/// small objects of scalars stay on one line (`"uv": [0, 0, 16, 16]`).
pub fn to_pretty(j: &J) -> String {
    fn scalar(j: &J) -> Option<String> {
        Some(match &j.v {
            V::Null => "null".into(),
            V::Bool(b) => b.to_string(),
            V::Num(n) => num_value(*n).to_string(),
            V::Str(s) => serde_json::Value::String(s.clone()).to_string(),
            _ => return None,
        })
    }
    fn inline(j: &J) -> Option<String> {
        if let Some(s) = scalar(j) {
            return Some(s);
        }
        let s = match &j.v {
            V::Arr(a) if a.iter().all(|i| matches!(i.v, V::Num(_) | V::Bool(_) | V::Null)) => {
                format!("[{}]", a.iter().filter_map(scalar).collect::<Vec<_>>().join(", "))
            }
            V::Obj(m) if !m.is_empty() && m.iter().all(|(_, v)| inline(v).is_some() && !matches!(v.v, V::Obj(_))) => {
                let parts: Vec<String> = m
                    .iter()
                    .filter_map(|(k, v)| Some(format!("{}: {}", serde_json::Value::String(k.clone()), inline(v)?)))
                    .collect();
                format!("{{ {} }}", parts.join(", "))
            }
            V::Obj(m) if m.is_empty() => "{}".into(),
            V::Arr(a) if a.is_empty() => "[]".into(),
            _ => return None,
        };
        (s.len() <= 72).then_some(s)
    }
    fn write(j: &J, indent: usize, out: &mut String) {
        if let Some(s) = inline(j) {
            out.push_str(&s);
            return;
        }
        let pad = "  ".repeat(indent + 1);
        match &j.v {
            V::Arr(a) => {
                out.push_str("[\n");
                for (i, item) in a.iter().enumerate() {
                    out.push_str(&pad);
                    write(item, indent + 1, out);
                    out.push_str(if i + 1 < a.len() { ",\n" } else { "\n" });
                }
                out.push_str(&"  ".repeat(indent));
                out.push(']');
            }
            V::Obj(m) => {
                out.push_str("{\n");
                for (i, (k, v)) in m.iter().enumerate() {
                    out.push_str(&pad);
                    out.push_str(&serde_json::Value::String(k.clone()).to_string());
                    out.push_str(": ");
                    write(v, indent + 1, out);
                    out.push_str(if i + 1 < m.len() { ",\n" } else { "\n" });
                }
                out.push_str(&"  ".repeat(indent));
                out.push('}');
            }
            _ => {}
        }
    }
    let mut out = String::new();
    write(j, 0, &mut out);
    out.push('\n');
    out
}

/// Parses `text`. Errors are `line N: message`. Duplicate keys are an error.
pub fn parse(text: &str) -> Result<J, String> {
    let mut p = P {
        s: text.as_bytes(),
        i: 0,
        line: 1,
    };
    if p.s.starts_with(b"\xEF\xBB\xBF") {
        p.i = 3;
    }
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i < p.s.len() {
        return Err(p.err("unexpected text after the JSON value"));
    }
    Ok(v)
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
    line: usize,
}

impl P<'_> {
    fn err(&self, msg: &str) -> String {
        format!("line {}: {msg}", self.line)
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn ws(&mut self) {
        while let Some(c) = self.peek() {
            match c {
                b'\n' => {
                    self.line += 1;
                    self.i += 1;
                }
                b' ' | b'\t' | b'\r' => self.i += 1,
                // `//` comments are tolerated (hand-edited files).
                b'/' if self.s.get(self.i + 1) == Some(&b'/') => {
                    while self.peek().is_some_and(|c| c != b'\n') {
                        self.i += 1;
                    }
                }
                _ => break,
            }
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn value(&mut self, depth: usize) -> Result<J, String> {
        if depth > 64 {
            return Err(self.err("nested too deeply"));
        }
        let line = self.line;
        let v = match self.peek() {
            Some(b'{') => {
                self.i += 1;
                let mut members: Vec<(String, J)> = Vec::new();
                self.ws();
                if !self.eat(b'}') {
                    loop {
                        self.ws();
                        let key_line = self.line;
                        if self.peek() != Some(b'"') {
                            return Err(self.err("expected a \"key\""));
                        }
                        let key = self.string()?;
                        if members.iter().any(|(k, _)| *k == key) {
                            return Err(format!("line {key_line}: duplicate key {key:?}"));
                        }
                        self.ws();
                        if !self.eat(b':') {
                            return Err(self.err("expected `:` after the key"));
                        }
                        self.ws();
                        let mut v = self.value(depth + 1)?;
                        // A member's line is its key's (where an editor shows it).
                        v.line = key_line;
                        members.push((key, v));
                        self.ws();
                        if self.eat(b',') {
                            self.ws();
                            // Trailing comma tolerated.
                            if self.eat(b'}') {
                                break;
                            }
                            continue;
                        }
                        if self.eat(b'}') {
                            break;
                        }
                        return Err(self.err("expected `,` or `}`"));
                    }
                }
                V::Obj(members)
            }
            Some(b'[') => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if !self.eat(b']') {
                    loop {
                        self.ws();
                        items.push(self.value(depth + 1)?);
                        self.ws();
                        if self.eat(b',') {
                            self.ws();
                            if self.eat(b']') {
                                break;
                            }
                            continue;
                        }
                        if self.eat(b']') {
                            break;
                        }
                        return Err(self.err("expected `,` or `]`"));
                    }
                }
                V::Arr(items)
            }
            Some(b'"') => V::Str(self.string()?),
            Some(b't') if self.s[self.i..].starts_with(b"true") => {
                self.i += 4;
                V::Bool(true)
            }
            Some(b'f') if self.s[self.i..].starts_with(b"false") => {
                self.i += 5;
                V::Bool(false)
            }
            Some(b'n') if self.s[self.i..].starts_with(b"null") => {
                self.i += 4;
                V::Null
            }
            Some(c) if c == b'-' || c.is_ascii_digit() => {
                let start = self.i;
                self.i += 1;
                while self
                    .peek()
                    .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-'))
                {
                    self.i += 1;
                }
                let lit = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("");
                V::Num(
                    lit.parse()
                        .map_err(|_| self.err(&format!("`{lit}` isn't a number")))?,
                )
            }
            None => return Err(self.err("unexpected end of file")),
            Some(_) => return Err(self.err("expected a JSON value")),
        };
        Ok(J { line, v })
    }

    fn string(&mut self) -> Result<String, String> {
        if !self.eat(b'"') {
            return Err(self.err("expected `\"`"));
        }
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.err("unterminated string"));
            };
            self.i += 1;
            match c {
                b'"' => break,
                b'\n' => return Err(self.err("line break inside a string (write \\n)")),
                b'\\' => {
                    let Some(e) = self.peek() else {
                        return Err(self.err("unterminated string"));
                    };
                    self.i += 1;
                    match e {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let mut code = self.hex4()?;
                            if (0xD800..0xDC00).contains(&code)
                                && self.s[self.i..].starts_with(b"\\u")
                            {
                                self.i += 2;
                                let low = self.hex4()?;
                                code = 0x10000 + ((code - 0xD800) << 10) + (low.wrapping_sub(0xDC00) & 0x3FF);
                            }
                            let ch = char::from_u32(code).unwrap_or('\u{FFFD}');
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return Err(self.err("unknown escape in a string")),
                    }
                }
                c => out.push(c),
            }
        }
        String::from_utf8(out).map_err(|_| self.err("invalid UTF-8 in a string"))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let hex = self
            .s
            .get(self.i..self.i + 4)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u32::from_str_radix(h, 16).ok())
            .ok_or_else(|| self.err("bad \\u escape"))?;
        self.i += 4;
        Ok(hex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_lines_of_members() {
        let j = parse("{\n  \"a\": 1,\n  \"b\": [\n    true,\n    \"x\"\n  ]\n}").unwrap();
        assert_eq!(j.line, 1);
        assert_eq!(j.get("a").unwrap().line, 2);
        let b = j.get("b").unwrap();
        assert_eq!(b.line, 3);
        assert_eq!(b.items()[1].line, 5);
        assert_eq!(b.items()[1].str(), Some("x"));
    }

    #[test]
    fn rejects_duplicates_and_reports_line() {
        let e = parse("{\n\"a\": 1,\n\"a\": 2}").unwrap_err();
        assert!(e.starts_with("line 3:"), "{e}");
    }

    #[test]
    fn tolerates_comments_and_trailing_commas() {
        let j = parse("{ // note\n \"a\": [1, 2,],\n}").unwrap();
        assert_eq!(j.get("a").unwrap().items().len(), 2);
    }

    #[test]
    fn pretty_prints_in_order() {
        let j = J::obj([("b", J::nums(&[0.0, 16.0])), ("a", J::obj([("x", J::string("y"))]))]);
        assert_eq!(to_pretty(&j), "{\n  \"b\": [0, 16],\n  \"a\": { \"x\": \"y\" }\n}\n");
        assert_eq!(parse(&to_pretty(&j)).unwrap().to_value(), j.to_value());
    }

    #[test]
    fn integral_numbers_stay_integral() {
        assert_eq!(num_value(16.0).to_string(), "16");
        assert_eq!(num_value(22.5).to_string(), "22.5");
    }
}
