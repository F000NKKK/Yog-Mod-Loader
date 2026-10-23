//! Types (`ys-grammar.md` §4): paths with arguments, tuples, nullables,
//! `A & B`; and telling a type ahead (a declaration) from an expression.

use super::{PResult, Parser};
use crate::ast::{Name, Type, TypeKind};
use crate::source::token::{Punct, Tok};

impl Parser<'_, '_, '_> {
    /// A type: `i32`, `List<T>`, `(i32 X, i32 Y)`, `Player?`, `Charge & Owned`.
    pub(crate) fn ty(&mut self) -> PResult<Type> {
        let start = self.span();
        let first = self.ty_one()?;
        if !self.at(Punct::Amp) {
            return Ok(first);
        }
        let mut all = vec![first];
        while self.eat(Punct::Amp) {
            all.push(self.ty_one()?);
        }
        Ok(Type {
            kind: TypeKind::Both(all),
            span: start.to(self.prev()),
        })
    }

    fn ty_one(&mut self) -> PResult<Type> {
        let start = self.span();
        let mut ty = if self.at(Punct::LParen) {
            self.ty_tuple()?
        } else {
            self.ty_path()?
        };
        if self.eat(Punct::Question) {
            ty = Type {
                kind: TypeKind::Nullable(Box::new(ty)),
                span: start.to(self.prev()),
            };
        }
        Ok(ty)
    }

    fn ty_path(&mut self) -> PResult<Type> {
        let start = self.span();
        let path = match self.tok() {
            Tok::Keyword(w) if w.is_type() => {
                let span = self.bump().span;
                vec![Name {
                    text: w.text.to_string(),
                    span,
                }]
            }
            _ => self.path("a type")?,
        };
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
        self.list(Punct::Gt, Self::ty)
    }

    /// `(i32 X, i32 Y)`.
    fn ty_tuple(&mut self) -> PResult<Type> {
        let start = self.need(Punct::LParen)?;
        let items = self.list(Punct::RParen, |p| {
            let ty = p.ty()?;
            let name = if matches!(p.tok(), Tok::Ident(_)) {
                Some(p.name("a name")?)
            } else {
                None
            };
            Ok((ty, name))
        })?;
        Ok(Type {
            kind: TypeKind::Tuple(items),
            span: start.to(self.prev()),
        })
    }

    /// How many tokens a type takes from `n` ahead, if one is written there.
    pub(crate) fn type_len(&self, n: usize) -> Option<usize> {
        let mut end = self.type_one_len(n)?;
        while self.nth_is(end, Punct::Amp) {
            end = self.type_one_len(end + 1)?;
        }
        Some(end - n)
    }

    /// Where one type (no `&`) starting at `n` ends.
    fn type_one_len(&self, n: usize) -> Option<usize> {
        let mut end = match self.nth(n) {
            Tok::Punct(Punct::LParen) => self.tuple_type_end(n)?,
            Tok::Keyword(w) if w.is_type() => n + 1,
            Tok::Ident(_) => {
                let mut end = n + 1;
                while self.nth_is(end, Punct::Dot) && matches!(self.nth(end + 1), Tok::Ident(_)) {
                    end += 2;
                }
                end
            }
            _ => return None,
        };
        if self.nth_is(end, Punct::Lt) {
            end = self.type_args_end(end)?;
        }
        if self.nth_is(end, Punct::Question) {
            end += 1;
        }
        Some(end)
    }

    /// Where `<…>` of types starting at `n` ends.
    fn type_args_end(&self, n: usize) -> Option<usize> {
        let mut end = n + 1;
        loop {
            end += self.type_len(end)?;
            match self.nth(end) {
                Tok::Punct(Punct::Comma) => end += 1,
                Tok::Punct(Punct::Gt) => return Some(end + 1),
                _ => return None,
            }
        }
    }

    /// Where `(T a, T b)` starting at `n` ends (two items at least).
    fn tuple_type_end(&self, n: usize) -> Option<usize> {
        let mut end = n + 1;
        let mut items = 0;
        loop {
            end += self.type_len(end)?;
            if matches!(self.nth(end), Tok::Ident(_)) {
                end += 1;
            }
            items += 1;
            match self.nth(end) {
                Tok::Punct(Punct::Comma) => end += 1,
                Tok::Punct(Punct::RParen) if items > 1 => return Some(end + 1),
                _ => return None,
            }
        }
    }

    /// Whether a declaration starts here: a type, then a name.
    pub(crate) fn typed_ahead(&self) -> Option<usize> {
        let len = self.type_len(0)?;
        matches!(self.nth(len), Tok::Ident(_)).then_some(len)
    }

    /// How many tokens `<…>` type arguments take from here, when they're
    /// type arguments (before `(`, `{`, `.`, `;`, `)`, `,`) and not a `<`.
    pub(crate) fn type_args_ahead(&self) -> Option<usize> {
        let end = self.type_args_end(0)?;
        let after = self.nth(end);
        let follows = matches!(
            after,
            Tok::Punct(
                Punct::LParen
                    | Punct::LBrace
                    | Punct::Dot
                    | Punct::Semi
                    | Punct::RParen
                    | Punct::Comma
            ) | Tok::Eof
        );
        follows.then_some(end)
    }
}
