//! The `.yui` XML subset — what `yog build` and Yog-IDLE read screens,
//! book pages and recipe layouts from: elements with attributes in source
//! order and their lines, for messages.

/// One XML element: name, attributes in source order, child elements.
#[derive(Debug, Clone)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
    /// 1-based line of the start tag, for error messages.
    pub line: usize,
}

impl Element {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
    /// The last [`Parser::line`] answer: (position, line there) — the
    /// parser only moves forward, so lines are counted from there on.
    line_at: std::cell::Cell<(usize, usize)>,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Parser {
            src,
            pos: 0,
            line_at: std::cell::Cell::new((0, 1)),
        }
    }

    fn line(&self) -> usize {
        let (at, line) = self.line_at.get();
        let (from, line) = if self.pos >= at { (at, line) } else { (0, 1) };
        let line = line
            + self.src.as_bytes()[from..self.pos]
                .iter()
                .filter(|&&b| b == b'\n')
                .count();
        self.line_at.set((self.pos, line));
        line
    }

    fn err<T>(&self, msg: impl std::fmt::Display) -> Result<T, String> {
        Err(format!("line {}: {msg}", self.line()))
    }

    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn skip_ws(&mut self) {
        let trimmed = self.rest().trim_start();
        self.pos = self.src.len() - trimmed.len();
    }

    /// Skips whitespace, comments, `<?...?>` and `<!DOCTYPE ...>`.
    fn skip_misc(&mut self) -> Result<(), String> {
        loop {
            self.skip_ws();
            let rest = self.rest();
            let end = if rest.starts_with("<!--") {
                rest.find("-->").map(|i| i + 3)
            } else if rest.starts_with("<?") {
                rest.find("?>").map(|i| i + 2)
            } else if rest.starts_with("<!") {
                rest.find('>').map(|i| i + 1)
            } else {
                return Ok(());
            };
            match end {
                Some(n) => self.pos += n,
                None => return self.err("unterminated comment or declaration"),
            }
        }
    }

    fn name(&mut self) -> Result<String, String> {
        let rest = self.rest();
        let len = rest
            .find(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.')))
            .unwrap_or(rest.len());
        if len == 0 {
            return self.err("expected a name");
        }
        self.pos += len;
        Ok(rest[..len].to_string())
    }

    /// Steps over `token` if the rest starts with it.
    fn eat(&mut self, token: &str) -> bool {
        let found = self.rest().starts_with(token);
        if found {
            self.pos += token.len();
        }
        found
    }

    fn element(&mut self) -> Result<Element, String> {
        let line = self.line();
        if !self.eat("<") {
            return self.err("expected an element");
        }
        let name = self.name()?;
        let mut attrs: Vec<(String, String)> = Vec::new();
        loop {
            self.skip_ws();
            if self.eat("/>") {
                return Ok(Element {
                    name,
                    attrs,
                    children: Vec::new(),
                    line,
                });
            }
            if self.eat(">") {
                break;
            }
            if self.rest().is_empty() {
                return self.err(format!("unterminated <{name}>"));
            }
            let (key, value) = self.attribute()?;
            if attrs.iter().any(|(k, _)| *k == key) {
                return self.err(format!("<{name}>: duplicate attribute `{key}`"));
            }
            attrs.push((key, value));
        }
        let children = self.children(&name)?;
        Ok(Element {
            name,
            attrs,
            children,
            line,
        })
    }

    /// `key="value"` (or single-quoted), unescaped.
    fn attribute(&mut self) -> Result<(String, String), String> {
        let key = self.name()?;
        self.skip_ws();
        if !self.eat("=") {
            return self.err(format!("expected `=` after attribute `{key}`"));
        }
        self.skip_ws();
        let quote = match self.rest().chars().next() {
            Some(q @ ('"' | '\'')) => q,
            _ => return self.err(format!("attribute `{key}`: expected a quoted value")),
        };
        self.pos += quote.len_utf8();
        let Some(end) = self.rest().find(quote) else {
            return self.err(format!("attribute `{key}`: unterminated value"));
        };
        let value =
            unescape(&self.rest()[..end]).map_err(|e| format!("line {}: {e}", self.line()))?;
        self.pos += end + quote.len_utf8();
        Ok((key, value))
    }

    /// The elements inside `<name>`, through its closing tag.
    fn children(&mut self, name: &str) -> Result<Vec<Element>, String> {
        let mut children = Vec::new();
        loop {
            self.skip_misc()?;
            if self.eat("</") {
                let close = self.name()?;
                if close != name {
                    return self.err(format!("</{close}> closes <{name}>"));
                }
                self.skip_ws();
                if !self.eat(">") {
                    return self.err("expected `>`");
                }
                return Ok(children);
            }
            let rest = self.rest();
            if rest.is_empty() {
                return self.err(format!("<{name}> is never closed"));
            }
            if !rest.starts_with('<') {
                return self.err(format!(
                    "<{name}>: text content isn't allowed — use a `text` attribute"
                ));
            }
            children.push(self.element()?);
        }
    }
}

fn unescape(raw: &str) -> Result<String, String> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let Some(semi) = rest[i..].find(';') else {
            return Err("`&` without `;` — write `&amp;`".into());
        };
        let entity = &rest[i + 1..i + semi];
        match entity {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                let code = if let Some(hex) = entity.strip_prefix("#x") {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse().ok()
                } else {
                    None
                };
                match code.and_then(char::from_u32) {
                    Some(c) => out.push(c),
                    None => return Err(format!("unknown entity `&{entity};`")),
                }
            }
        }
        rest = &rest[i + semi + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Parses an XML document (the subset `.yui` uses) into its root element.
pub fn parse(src: &str) -> Result<Element, String> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut p = Parser::new(src);
    p.skip_misc()?;
    let root = p.element()?;
    p.skip_misc()?;
    if !p.rest().is_empty() {
        return p.err("content after the root element");
    }
    Ok(root)
}
