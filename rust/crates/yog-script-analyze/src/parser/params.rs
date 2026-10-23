//! Names, parameter and argument lists, strings and numbers as forms take
//! them.

use super::{PResult, Parser};
use crate::ast::{Arg, Name, Param};
use crate::source::token::{Punct, StrPart, Tok};

impl Parser<'_, '_, '_> {
    /// A name (not a reserved word).
    pub(crate) fn name(&mut self, what: &str) -> PResult<Name> {
        if let Tok::Ident(text) = self.tok() {
            let span = self.bump().span;
            return Ok(Name {
                text: text.clone(),
                span,
            });
        }
        Err(self.expected(what))
    }

    /// A name where reserved words are names too (after `.`, in event paths).
    pub(crate) fn any_name(&mut self, what: &str) -> PResult<Name> {
        if let Tok::Keyword(w) = self.tok() {
            let span = self.bump().span;
            return Ok(Name {
                text: w.text.to_string(),
                span,
            });
        }
        self.name(what)
    }

    /// `A.B.C`.
    pub(crate) fn path(&mut self, what: &str) -> PResult<Vec<Name>> {
        let mut path = vec![self.any_name(what)?];
        while self.at(Punct::Dot) && matches!(self.nth(1), Tok::Ident(_) | Tok::Keyword(_)) {
            self.bump();
            path.push(self.any_name(what)?);
        }
        Ok(path)
    }

    /// Doc comments skipped (where nothing they could document follows).
    pub(crate) fn skip_docs(&mut self) {
        while matches!(self.tok(), Tok::Doc(_)) {
            self.bump();
        }
    }

    /// `(Type name = default, …)`.
    pub(crate) fn params(&mut self) -> PResult<Vec<Param>> {
        self.need(Punct::LParen)?;
        self.braces(|p| p.list(Punct::RParen, Self::param))
    }

    fn param(&mut self) -> PResult<Param> {
        let ty = self.ty()?;
        let name = self.name("a parameter's name")?;
        let default = if self.eat(Punct::Eq) {
            Some(self.expr()?)
        } else {
            None
        };
        Ok(Param { ty, name, default })
    }

    /// `(a, name: b)`.
    pub(crate) fn args(&mut self) -> PResult<Vec<Arg>> {
        self.need(Punct::LParen)?;
        self.braces(|p| p.list(Punct::RParen, Self::arg))
    }

    /// `name: value` or `value`.
    fn arg(&mut self) -> PResult<Arg> {
        let named = matches!(self.tok(), Tok::Ident(_)) && self.nth_is(1, Punct::Colon);
        let name = if named {
            let name = self.name("an argument")?;
            self.bump();
            Some(name)
        } else {
            None
        };
        Ok(Arg {
            name,
            value: self.expr()?,
        })
    }

    /// `<T, U>`, or nothing.
    pub(crate) fn generics(&mut self) -> PResult<Vec<Name>> {
        if !self.eat(Punct::Lt) {
            return Ok(Vec::new());
        }
        self.list(Punct::Gt, |p| p.name("a type parameter"))
    }

    /// Items by `item`, separated by `,` (a trailing one allowed), up to
    /// and with `close`.
    pub(crate) fn list<T>(
        &mut self,
        close: Punct,
        mut item: impl FnMut(&mut Self) -> PResult<T>,
    ) -> PResult<Vec<T>> {
        let mut out = Vec::new();
        loop {
            if self.eat(close) {
                return Ok(out);
            }
            out.push(item(self)?);
            if !self.eat(Punct::Comma) {
                self.need(close)?;
                return Ok(out);
            }
        }
    }

    /// A string without `{…}` in it (a file, a test's name).
    pub(crate) fn plain_string(&mut self, what: &str) -> PResult<String> {
        let Tok::Str(parts) = self.tok() else {
            return Err(self.expected(what));
        };
        let span = self.bump().span;
        let mut text = String::new();
        for part in parts {
            match part {
                StrPart::Text(t) => text.push_str(t),
                StrPart::Expr(_) => {
                    let code = yog_ids::script_problem::EXPECTED;
                    return Err(self.error(code, span, format!("{what} can't have `{{…}}` in it")));
                }
            }
        }
        Ok(text)
    }

    /// A version as written (`1.20.1`, `1.21`, `26`).
    pub(crate) fn version(&mut self) -> PResult<String> {
        if matches!(self.tok(), Tok::Version(_) | Tok::Float(_) | Tok::Int(_)) {
            let span = self.bump().span;
            return Ok(self.report.text(span).to_string());
        }
        Err(self.expected("a version"))
    }

    pub(crate) fn int(&mut self, what: &str) -> PResult<i64> {
        if let Tok::Int(v) = self.tok() {
            self.bump();
            return Ok(*v);
        }
        Err(self.expected(what))
    }
}
