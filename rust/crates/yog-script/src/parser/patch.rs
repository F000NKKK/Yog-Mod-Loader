//! The parts patch words share: a target method's signature, a callee, a
//! selector; and the turn-taking inside a
//! patch's and a method's braces.

use super::{PResult, Parser};
use crate::ast::{Callee, Method, PatchItem, Point, Selector};
use crate::token::{Punct, Tok};
use crate::words::{self, ALL, Role};

impl Parser<'_, '_, '_> {
    /// `name(params) (-> Type)?`.
    pub(crate) fn method(&mut self) -> PResult<Method> {
        let name = self.name("a method")?;
        let params = self.params()?;
        let ret = if self.eat(Punct::Arrow) {
            Some(self.ty_plain()?)
        } else {
            None
        };
        Ok(Method { name, params, ret })
    }

    pub(crate) fn int(&mut self, what: &str) -> PResult<i64> {
        if let Tok::Int(v) = self.tok() {
            self.bump();
            return Ok(*v);
        }
        Err(self.expected(what))
    }

    /// `aiStep`, `Vec3.add`, `Vec3.add(Double, Double)`.
    pub(crate) fn callee(&mut self) -> PResult<Callee> {
        let path = self.path("a method")?;
        let args = if self.eat(Punct::LParen) {
            Some(self.list(Punct::RParen, Self::ty_plain)?)
        } else {
            None
        };
        Ok(Callee { path, args })
    }

    /// `first`, `last`, `where cond`, or none.
    pub(crate) fn selector(&mut self) -> PResult<Option<Selector>> {
        if self.eat_word(&words::first::WORD) {
            return Ok(Some(Selector::First));
        }
        if self.eat_word(&words::last::WORD) {
            return Ok(Some(Selector::Last));
        }
        if self.eat_word(&words::r#where::WORD) {
            return Ok(Some(Selector::Where(self.expr_no_brace()?)));
        }
        Ok(None)
    }

    /// `{ items }` of a patch, each by its word.
    pub(crate) fn patch_items(&mut self) -> PResult<Vec<PatchItem>> {
        self.turns(
            |w| {
                w.role(|r| {
                    if let Role::PatchItem { parse } = r {
                        Some(*parse)
                    } else {
                        None
                    }
                })
            },
            "a patch item",
        )
    }

    /// `{ points }` inside a method, each by its word.
    pub(crate) fn points(&mut self) -> PResult<Vec<Point>> {
        self.turns(
            |w| {
                w.role(|r| {
                    if let Role::Point { parse } = r {
                        Some(*parse)
                    } else {
                        None
                    }
                })
            },
            "a point in the method",
        )
    }

    /// `{ … }` whose every line starts with a word `role_of` gives a parser.
    fn turns<T>(
        &mut self,
        role_of: impl Fn(&words::Word) -> Option<fn(&mut Parser) -> PResult<T>>,
        what: &str,
    ) -> PResult<Vec<T>> {
        self.need(Punct::LBrace)?;
        let mut out = Vec::new();
        loop {
            self.skip_newlines();
            if self.eat(Punct::RBrace) {
                return Ok(out);
            }
            if *self.tok() == Tok::Eof {
                return Err(self.expected("`}`"));
            }
            let parse = ALL
                .iter()
                .filter(|w| self.at_word(w))
                .find_map(|w| role_of(w));
            let item = match parse {
                Some(parse) => parse(self).and_then(|i| self.end_stmt().map(|()| i)),
                None => Err(self.expected(what)),
            };
            match item {
                Ok(i) => out.push(i),
                Err(_) => self.recover(),
            }
        }
    }
}
