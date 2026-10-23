//! The parts patch words share: a target method's signature, a callee, a
//! selector; and the turn-taking inside a patch's and a method's braces.

use super::{PResult, Parser};
use crate::ast::{Callee, PatchItem, Point, Selector, Target};
use crate::source::token::{Punct, Tok};
use crate::words::{self, ALL, Role};

impl Parser<'_, '_, '_> {
    /// `bool Hurt(DamageSource source, float amount)`, `void Tick()`.
    pub(crate) fn target(&mut self) -> PResult<Target> {
        let ret = if self.eat_word(&words::void::WORD) {
            None
        } else {
            Some(self.ty()?)
        };
        let name = self.any_name("a method")?;
        let params = self.params()?;
        Ok(Target { ret, name, params })
    }

    /// `AiStep`, `Vec3.Add`, `Vec3.Add(double, double)`.
    pub(crate) fn callee(&mut self) -> PResult<Callee> {
        let path = self.path("a method")?;
        let args = if self.at(Punct::LParen) && !self.lone_name_in_parens() {
            self.bump();
            Some(self.list(Punct::RParen, Self::ty)?)
        } else {
            None
        };
        Ok(Callee { path, args })
    }

    /// `(x)` before `{`: an argument's name, not a list of types.
    pub(crate) fn lone_name_in_parens(&self) -> bool {
        matches!(self.nth(1), Tok::Ident(_))
            && self.nth_is(2, Punct::RParen)
            && self.nth_is(3, Punct::LBrace)
    }

    /// `first`, `last`, `where (cond)`, or none.
    pub(crate) fn selector(&mut self) -> PResult<Option<Selector>> {
        if self.eat_word(&words::first::WORD) {
            return Ok(Some(Selector::First));
        }
        if self.eat_word(&words::last::WORD) {
            return Ok(Some(Selector::Last));
        }
        if self.eat_word(&words::r#where::WORD) {
            return Ok(Some(Selector::Where(self.paren_cond()?)));
        }
        Ok(None)
    }

    /// `{ items }` of a patch, each by its word.
    pub(crate) fn patch_items(&mut self) -> PResult<Vec<PatchItem>> {
        self.turns(
            |w| {
                w.role(|r| match r {
                    Role::PatchItem { parse } => Some(*parse),
                    _ => None,
                })
            },
            "a patch item",
        )
    }

    /// `{ points }` inside a method, each by its word.
    pub(crate) fn points(&mut self) -> PResult<Vec<Point>> {
        self.turns(
            |w| {
                w.role(|r| match r {
                    Role::Point { parse } => Some(*parse),
                    _ => None,
                })
            },
            "a point in the method",
        )
    }

    /// `{ … }` whose every item starts with a word `role_of` gives a parser.
    fn turns<T>(
        &mut self,
        role_of: impl Fn(&words::Word) -> Option<fn(&mut Parser) -> PResult<T>>,
        what: &str,
    ) -> PResult<Vec<T>> {
        self.need(Punct::LBrace)?;
        let mut out = Vec::new();
        loop {
            self.skip_docs();
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
                Some(parse) => parse(self),
                None => Err(self.expected(what)),
            };
            match item {
                Ok(i) => out.push(i),
                Err(_) => self.recover(),
            }
        }
    }
}
