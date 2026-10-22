//! Patterns (`ys-grammar.md` §3.3): `let`, `for`, `match` arms.

use super::{PResult, Parser};
use crate::ast::{Expr, ExprKind, Pattern};
use crate::token::{Punct, Tok};
use crate::words;

const WILDCARD: &str = "_";

impl Parser<'_, '_, '_> {
    /// One pattern, or several joined by `|`.
    pub(crate) fn pattern(&mut self) -> PResult<Pattern> {
        let first = self.pattern_one()?;
        if !self.at(Punct::Pipe) {
            return Ok(first);
        }
        let mut all = vec![first];
        while self.eat(Punct::Pipe) {
            all.push(self.pattern_one()?);
        }
        Ok(Pattern::Or(all))
    }

    fn pattern_one(&mut self) -> PResult<Pattern> {
        if self.at_word(&words::r#else::WORD) {
            return Ok(Pattern::Else(self.bump().span));
        }
        if self.eat_word(&words::is::WORD) {
            let ty = self.ty_plain()?;
            let bind = if matches!(self.tok(), Tok::Ident(_)) {
                Some(self.name("a name")?)
            } else {
                None
            };
            return Ok(Pattern::Is { ty, bind });
        }
        match self.tok() {
            Tok::Ident(name) if name == WILDCARD => Ok(Pattern::Wild(self.bump().span)),
            Tok::Ident(_) => self.path_pattern(),
            Tok::Punct(Punct::LParen) => {
                self.bump();
                Ok(Pattern::Tuple(self.list(Punct::RParen, Self::pattern)?))
            }
            _ => self.value_pattern(),
        }
    }

    /// `Circle(r)`, or a name or path to a value (`energy.max`).
    fn path_pattern(&mut self) -> PResult<Pattern> {
        let start = self.span();
        let path = self.path("a pattern")?;
        if self.eat(Punct::LParen) {
            let args = self.list(Punct::RParen, Self::pattern)?;
            return Ok(Pattern::Variant { path, args });
        }
        let mut names = path.into_iter();
        let first = names.next().map(|n| Expr {
            kind: ExprKind::Ident(n.text),
            span: n.span,
        });
        let mut expr = first.ok_or_else(|| self.expected("a pattern"))?;
        for name in names {
            let span = start.to(name.span);
            expr = Expr {
                kind: ExprKind::Member {
                    expr: Box::new(expr),
                    name,
                    safe: false,
                },
                span,
            };
        }
        Ok(Pattern::Value(expr))
    }

    /// A literal, or a range of two.
    fn value_pattern(&mut self) -> PResult<Pattern> {
        let from = self.unary()?;
        if !self.eat(Punct::Range) {
            return Ok(Pattern::Value(from));
        }
        let to = self.unary()?;
        Ok(Pattern::Range { from, to })
    }
}
