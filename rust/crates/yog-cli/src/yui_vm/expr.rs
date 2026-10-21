//! A binding's expression (`{x:Bind vm.state?.name}`): read into a small
//! tree of paths, calls, literals, `!`, `==` and `!=`, written out as Rust
//! that reads the view model.

use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Tok {
    Ident(String),
    Num(String),
    Str(String),
    PathSep,
    Dot,
    OptDot,
    Open,
    Close,
    Comma,
    Eq,
    Ne,
    Not,
}

/// The punctuation an expression takes, the two-character ones first.
const SYMBOLS: [(&str, Tok); 9] = [
    ("::", Tok::PathSep),
    ("?.", Tok::OptDot),
    ("==", Tok::Eq),
    ("!=", Tok::Ne),
    ("!", Tok::Not),
    (".", Tok::Dot),
    ("(", Tok::Open),
    (")", Tok::Close),
    (",", Tok::Comma),
];

/// What quotes a string in a binding: `'text'`.
const QUOTE: char = '\'';

/// Reads an expression's characters into tokens.
struct Lexer {
    cs: Vec<char>,
    i: usize,
}

impl Lexer {
    /// The characters from `start` up to where `more` stops holding.
    fn take_while(&mut self, start: usize, more: impl Fn(&[char], usize) -> bool) -> String {
        while self.i < self.cs.len() && more(&self.cs, self.i) {
            self.i += 1;
        }
        self.cs[start..self.i].iter().collect()
    }

    fn number(&mut self) -> Tok {
        let s = self.i;
        self.i += 1;
        Tok::Num(self.take_while(s, |cs, i| {
            cs[i].is_ascii_alphanumeric()
                || cs[i] == '_'
                || (cs[i] == '.' && cs.get(i + 1).is_some_and(|n| n.is_ascii_digit()))
        }))
    }

    fn string(&mut self) -> Result<Tok, String> {
        self.i += 1;
        let text = self.take_while(self.i, |cs, i| cs[i] != QUOTE);
        if self.i >= self.cs.len() {
            return Err("unterminated 'string'".into());
        }
        self.i += 1;
        Ok(Tok::Str(text))
    }

    /// The punctuation here, if any.
    fn symbol(&mut self) -> Option<Tok> {
        let rest: String = self.cs[self.i..].iter().take(2).collect();
        let (s, tok) = SYMBOLS.iter().find(|(s, _)| rest.starts_with(s))?;
        self.i += s.chars().count();
        Some(tok.clone())
    }

    fn next(&mut self) -> Result<Option<Tok>, String> {
        while self.cs.get(self.i).is_some_and(|c| c.is_whitespace()) {
            self.i += 1;
        }
        let Some(&c) = self.cs.get(self.i) else {
            return Ok(None);
        };
        let next = self.cs.get(self.i + 1).copied();
        Ok(Some(match c {
            _ if c.is_alphabetic() || c == '_' => Tok::Ident(self.take_while(self.i, |cs, i| cs[i].is_alphanumeric() || cs[i] == '_')),
            _ if c.is_ascii_digit() || (c == '-' && next.is_some_and(|n| n.is_ascii_digit())) => self.number(),
            QUOTE => self.string()?,
            _ => self.symbol().ok_or_else(|| format!("unexpected `{c}` (bindings allow paths, calls, literals, !, == and !=; use a view-model method for anything else)"))?,
        }))
    }
}

pub(super) fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let mut lexer = Lexer {
        cs: src.chars().collect(),
        i: 0,
    };
    let mut out = Vec::new();
    while let Some(t) = lexer.next()? {
        out.push(t);
    }
    Ok(out)
}

/// A `.name` or `?.name(args)` after the head of a path.
#[derive(Debug, Clone)]
pub(super) struct Seg {
    pub(super) opt: bool,
    pub(super) name: String,
    pub(super) args: Option<Vec<Ex>>,
}

/// What a path starts with: a name (a call), a type path, a literal, `( … )`.
#[derive(Debug, Clone)]
pub(super) enum Head {
    Ident(String, Option<Vec<Ex>>),
    Type(String, Option<Vec<Ex>>),
    Lit(String),
    Paren(Box<Ex>),
}

#[derive(Debug, Clone)]
pub(super) enum Ex {
    Post(Head, Vec<Seg>),
    Not(Box<Ex>),
    Cmp(Box<Ex>, &'static str, Box<Ex>),
}

/// The names an expression's roots are: the view model, the command's
/// context, a template's item and its index.
pub(super) const VM: &str = "vm";
pub(super) const CX: &str = "cx";
pub(super) const ITEM: &str = "item";
pub(super) const INDEX: &str = "index";
/// The path separator, in a type path.
const PATH_SEP: &str = "::";

struct ExParser {
    toks: Vec<Tok>,
    pos: usize,
}

impl ExParser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn eat(&mut self, t: &Tok) -> bool {
        let here = self.peek() == Some(t);
        if here {
            self.pos += 1;
        }
        here
    }

    fn expr(&mut self) -> Result<Ex, String> {
        let a = self.unary()?;
        let op = if self.eat(&Tok::Eq) {
            "=="
        } else if self.eat(&Tok::Ne) {
            "!="
        } else {
            return Ok(a);
        };
        let b = self.unary()?;
        Ok(Ex::Cmp(Box::new(a), op, Box::new(b)))
    }

    fn unary(&mut self) -> Result<Ex, String> {
        if self.eat(&Tok::Not) {
            return Ok(Ex::Not(Box::new(self.postfix()?)));
        }
        self.postfix()
    }

    fn call(&mut self) -> Result<Option<Vec<Ex>>, String> {
        if !self.eat(&Tok::Open) {
            return Ok(None);
        }
        let mut args = Vec::new();
        if self.eat(&Tok::Close) {
            return Ok(Some(args));
        }
        loop {
            args.push(self.expr()?);
            if self.eat(&Tok::Close) {
                return Ok(Some(args));
            }
            if !self.eat(&Tok::Comma) {
                return Err("expected `,` or `)`".into());
            }
        }
    }

    /// A name, a type path (`Type::f(x)`), `true`/`false`.
    fn named(&mut self, id: String) -> Result<Head, String> {
        if self.peek() != Some(&Tok::PathSep) {
            if id == yog_ids::yui_value::TRUE || id == yog_ids::yui_value::FALSE {
                return Ok(Head::Lit(id));
            }
            return Ok(Head::Ident(id, self.call()?));
        }
        let mut path = id;
        while self.eat(&Tok::PathSep) {
            let Some(Tok::Ident(n)) = self.bump() else {
                return Err("expected a name after `::`".into());
            };
            path.push_str(PATH_SEP);
            path.push_str(&n);
        }
        Ok(Head::Type(path, self.call()?))
    }

    fn head(&mut self) -> Result<Head, String> {
        match self.bump() {
            Some(Tok::Ident(id)) => self.named(id),
            Some(Tok::Num(n)) => Ok(Head::Lit(n)),
            Some(Tok::Str(s)) => Ok(Head::Lit(format!("{s:?}"))),
            Some(Tok::Open) => {
                let e = self.expr()?;
                if !self.eat(&Tok::Close) {
                    return Err("expected `)`".into());
                }
                Ok(Head::Paren(Box::new(e)))
            }
            Some(t) => Err(format!("unexpected {t:?}")),
            None => Err("expected an expression".into()),
        }
    }

    fn postfix(&mut self) -> Result<Ex, String> {
        let head = self.head()?;
        let mut segs = Vec::new();
        loop {
            let opt = if self.eat(&Tok::Dot) {
                false
            } else if self.eat(&Tok::OptDot) {
                true
            } else {
                break;
            };
            let Some(Tok::Ident(name)) = self.bump() else {
                return Err("expected a name after `.`".into());
            };
            let args = self.call()?;
            segs.push(Seg { opt, name, args });
        }
        Ok(Ex::Post(head, segs))
    }
}

pub(super) fn parse_expr(src: &str) -> Result<Ex, String> {
    let mut p = ExParser {
        toks: lex(src)?,
        pos: 0,
    };
    let e = p.expr()?;
    if let Some(t) = p.toks.get(p.pos) {
        return Err(format!("unexpected {t:?}"));
    }
    Ok(e)
}

/// Which names a binding sees.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Scope {
    Screen,
    Template,
}

impl Ex {
    /// Whether it has a `?.` anywhere: it reads as an `Option`.
    pub(super) fn has_opt(&self) -> bool {
        match self {
            Ex::Post(h, segs) => {
                let head = match h {
                    Head::Ident(_, a) | Head::Type(_, a) => a.iter().flatten().any(Ex::has_opt),
                    Head::Paren(e) => e.has_opt(),
                    Head::Lit(_) => false,
                };
                head || segs
                    .iter()
                    .any(|s| s.opt || s.args.iter().flatten().any(Ex::has_opt))
            }
            Ex::Not(e) => e.has_opt(),
            Ex::Cmp(a, _, b) => a.has_opt() || b.has_opt(),
        }
    }

    /// A field path (`search`, `vm.state.name`, `item.id`): assignable, and
    /// passed by reference to type-path calls.
    pub(super) fn is_place(&self) -> bool {
        match self {
            Ex::Post(Head::Ident(n, None), segs) => {
                n != CX && segs.iter().all(|s| !s.opt && s.args.is_none())
            }
            _ => false,
        }
    }

    /// A call's arguments as Rust: `(a, &b)` — a place by reference when `by_ref`.
    fn args(
        a: &Option<Vec<Ex>>,
        by_ref: bool,
        scope: Scope,
        command: bool,
    ) -> Result<String, String> {
        let Some(a) = a else { return Ok(String::new()) };
        let parts = a
            .iter()
            .map(|e| {
                let s = e.emit(scope, command)?;
                Ok(if by_ref && e.is_place() {
                    format!("&{s}")
                } else {
                    s
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(format!("({})", parts.join(", ")))
    }

    /// A bare name's root in `scope`: the view model's field, the item's,
    /// or one of the names a scope has.
    fn root(n: &str, scope: Scope, command: bool) -> Result<String, String> {
        Ok(match (n, scope) {
            (VM, _) => VM.to_string(),
            (CX, _) if command => CX.to_string(),
            (CX, _) => return Err("`cx` is only available in commands".into()),
            (ITEM | INDEX, Scope::Template) => n.to_string(),
            (ITEM | INDEX, Scope::Screen) => {
                return Err(format!("`{n}` is only available in item templates"));
            }
            (_, Scope::Screen) => format!("{VM}.{n}"),
            (_, Scope::Template) => format!("{ITEM}.{n}"),
        })
    }

    /// The expression as Rust in `scope` (`command`: in a command's body,
    /// where `cx` is).
    pub(super) fn emit(&self, scope: Scope, command: bool) -> Result<String, String> {
        Ok(match self {
            Ex::Post(h, segs) => {
                let mut s = match h {
                    Head::Lit(l) => l.clone(),
                    Head::Paren(e) => format!("({})", e.emit(scope, command)?),
                    Head::Type(p, a) => format!("{p}{}", Ex::args(a, true, scope, command)?),
                    Head::Ident(n, a) => format!(
                        "{}{}",
                        Ex::root(n, scope, command)?,
                        Ex::args(a, false, scope, command)?
                    ),
                };
                for seg in segs {
                    if seg.opt {
                        s.push('?');
                    }
                    let _ = write!(
                        s,
                        ".{}{}",
                        seg.name,
                        Ex::args(&seg.args, false, scope, command)?
                    );
                }
                s
            }
            Ex::Not(e) => format!("!{}", e.emit(scope, command)?),
            Ex::Cmp(a, op, b) => format!(
                "{} {op} {}",
                a.emit(scope, command)?,
                b.emit(scope, command)?
            ),
        })
    }
}
