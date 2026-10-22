//! Names, parameter lists and generic parameters.

use super::{PResult, Parser};
use crate::ast::{Generic, Name, Param};
use crate::token::{Punct, StrPart, Tok};
use crate::words;

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

    /// `a.b.c`.
    pub(crate) fn path(&mut self, what: &str) -> PResult<Vec<Name>> {
        let mut path = vec![self.any_name(what)?];
        while self.at(Punct::Dot) && matches!(self.nth(1), Tok::Ident(_) | Tok::Keyword(_)) {
            self.bump();
            path.push(self.any_name(what)?);
        }
        Ok(path)
    }

    /// `(var? name (: Type)? (= default)?, …)`.
    pub(crate) fn params(&mut self) -> PResult<Vec<Param>> {
        self.need(Punct::LParen)?;
        self.braces(|p| p.list(Punct::RParen, Self::param))
    }

    fn param(&mut self) -> PResult<Param> {
        let var = self.eat_word(&words::r#var::WORD);
        let name = self.name("a parameter")?;
        let ty = if self.eat(Punct::Colon) {
            Some(self.ty()?)
        } else {
            None
        };
        let default = if self.eat(Punct::Eq) {
            Some(self.expr()?)
        } else {
            None
        };
        Ok(Param {
            var,
            name,
            ty,
            default,
        })
    }

    /// `(a, b)`, or nothing: the names an event or a lambda hands over.
    pub(crate) fn untyped_params(&mut self) -> PResult<Vec<Name>> {
        if !self.eat(Punct::LParen) {
            return Ok(Vec::new());
        }
        self.list(Punct::RParen, |p| p.name("a parameter"))
    }

    /// `<T, U: Base>`, or nothing.
    pub(crate) fn generics(&mut self) -> PResult<Vec<Generic>> {
        if !self.eat(Punct::Lt) {
            return Ok(Vec::new());
        }
        self.list(Punct::Gt, |p| {
            let name = p.name("a type parameter")?;
            let bound = if p.eat(Punct::Colon) {
                Some(p.ty()?)
            } else {
                None
            };
            Ok(Generic { name, bound })
        })
    }

    /// Items by `item`, separated by `,` (a trailing one allowed, newlines
    /// between them too), up to and with `close`.
    pub(crate) fn list<T>(
        &mut self,
        close: Punct,
        mut item: impl FnMut(&mut Self) -> PResult<T>,
    ) -> PResult<Vec<T>> {
        let mut out = Vec::new();
        loop {
            self.skip_newlines();
            if self.eat(close) {
                return Ok(out);
            }
            out.push(item(self)?);
            self.skip_newlines();
            if !self.eat(Punct::Comma) {
                self.skip_newlines();
                self.need(close)?;
                return Ok(out);
            }
        }
    }

    /// A string without `{…}` in it (a file, a test's name, a recipe row).
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
}
