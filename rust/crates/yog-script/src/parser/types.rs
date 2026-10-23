//! Types (`ys-grammar.md` §4): paths with arguments, tuples, functions,
//! optionals, bounded (`Int in 0..100`).

use super::{PResult, Parser};
use crate::ast::{Type, TypeKind};
use crate::source::token::{Punct, Tok};
use crate::words::{self, Level};

impl Parser<'_, '_, '_> {
    /// A type, bounded by a range where one follows (`Int in 0..100`).
    pub(crate) fn ty(&mut self) -> PResult<Type> {
        let ty = self.ty_plain()?;
        if !self.eat_word(&words::r#in::WORD) {
            return Ok(ty);
        }
        let range = self.binary(Level::Range)?;
        let span = ty.span.to(range.span);
        Ok(Type {
            kind: TypeKind::Bounded {
                base: Box::new(ty),
                range: Box::new(range),
            },
            span,
        })
    }

    /// A type without a bound (after `is`, `as`, in a tuple).
    pub(crate) fn ty_plain(&mut self) -> PResult<Type> {
        let start = self.span();
        let mut ty = if self.at(Punct::LParen) {
            self.ty_paren()?
        } else {
            self.ty_path()?
        };
        while self.at(Punct::Question) {
            self.bump();
            ty = Type {
                kind: TypeKind::Optional(Box::new(ty)),
                span: start.to(self.prev()),
            };
        }
        Ok(ty)
    }

    fn ty_path(&mut self) -> PResult<Type> {
        let start = self.span();
        let path = self.path("a type")?;
        let args = if self.at(Punct::Lt) {
            self.type_args()?
        } else {
            Vec::new()
        };
        Ok(Type {
            kind: TypeKind::Path { path, args },
            span: start.to(self.prev()),
        })
    }

    /// `<A, B>`.
    pub(crate) fn type_args(&mut self) -> PResult<Vec<Type>> {
        self.need(Punct::Lt)?;
        self.list(Punct::Gt, Self::ty_plain)
    }

    /// `(A, B)`, `(A)`, `(A, B) -> C`, `() -> C`.
    fn ty_paren(&mut self) -> PResult<Type> {
        let start = self.need(Punct::LParen)?;
        let items = self.list(Punct::RParen, Self::ty_plain)?;
        if self.eat(Punct::Arrow) {
            let ret = Box::new(self.ty_plain()?);
            return Ok(Type {
                kind: TypeKind::Fn { params: items, ret },
                span: start.to(self.prev()),
            });
        }
        match <[Type; 1]>::try_from(items) {
            Ok([one]) => Ok(one),
            Err(items) => Ok(Type {
                kind: TypeKind::Tuple(items),
                span: start.to(self.prev()),
            }),
        }
    }

    /// How many tokens `<…>` type arguments take from here, when they're
    /// type arguments (before `(`, `{`, `.` or the end) and not a `<`.
    pub(crate) fn type_args_ahead(&self) -> Option<usize> {
        let mut depth = 0usize;
        let mut n = 0;
        loop {
            match self.nth(n) {
                Tok::Punct(Punct::Lt) => depth += 1,
                Tok::Punct(Punct::Gt) => {
                    depth -= 1;
                    if depth == 0 {
                        let after = self.nth(n + 1);
                        let call = matches!(
                            after,
                            Tok::Punct(Punct::LParen | Punct::LBrace | Punct::Dot)
                        ) || matches!(after, Tok::Newline | Tok::Eof);
                        return call.then_some(n + 1);
                    }
                }
                Tok::Ident(_) | Tok::Punct(Punct::Dot | Punct::Comma | Punct::Question) => {}
                _ => return None,
            }
            n += 1;
        }
    }
}
