//! Types (`ys-grammar.md` §4): paths with arguments, tuples, nullables,
//! `A & B`; and telling a type ahead (a declaration) from an expression.

use super::{PResult, Parser};
use crate::ast::{Name, Type, TypeKind};
use crate::source::token::{Punct, Tok};
use crate::words::{r#in, r#ref};

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
        let borrow = [(&r#in::WORD, false), (&r#ref::WORD, true)]
            .into_iter()
            .find(|(w, _)| self.at_word(w));
        if let Some((_, mutable)) = borrow {
            self.bump();
            let lifetime = self.lifetime();
            let ty = Box::new(self.ty_one()?);
            return Ok(Type {
                kind: TypeKind::Borrow {
                    mutable,
                    lifetime,
                    ty,
                },
                span: start.to(self.prev()),
            });
        }
        if let Some(name) = self.lifetime() {
            return Ok(Type {
                kind: TypeKind::Lifetime(name),
                span: start,
            });
        }
        let mut ty = if self.at(Punct::LParen) {
            self.ty_tuple()?
        } else {
            self.ty_path()?
        };
        ty = self.nullable_of(ty, start);
        let ranks = self.ranks();
        ty = Self::array_of(ty, &ranks, start.to(self.prev()));
        Ok(self.nullable_of(ty, start))
    }

    /// `ty?` when a `?` follows.
    fn nullable_of(&mut self, ty: Type, start: crate::source::token::Span) -> Type {
        if !self.eat(Punct::Question) {
            return ty;
        }
        Type {
            kind: TypeKind::Nullable(Box::new(ty)),
            span: start.to(self.prev()),
        }
    }

    /// The ranks of the `[]` / `[,]` here, read.
    pub(crate) fn ranks(&mut self) -> Vec<usize> {
        let mut ranks = Vec::new();
        while let Some(rank) = self.rank_ahead(0) {
            (0..=rank).for_each(|_| {
                self.bump();
            });
            ranks.push(rank);
        }
        ranks
    }

    /// `item` with `ranks` after it, as C# reads them: the first the
    /// outermost (`i32[][,]` is an array of `i32[,]`).
    pub(crate) fn array_of(item: Type, ranks: &[usize], span: crate::source::token::Span) -> Type {
        ranks.iter().rev().fold(item, |t, &rank| Type {
            kind: TypeKind::Array {
                item: Box::new(t),
                rank,
            },
            span,
        })
    }

    /// `[]` (1) or `[,]` (2), … starting `n` ahead: an array's rank; its
    /// tokens are the rank and one.
    pub(crate) fn rank_ahead(&self, n: usize) -> Option<usize> {
        if !self.nth_is(n, Punct::LBracket) {
            return None;
        }
        let mut rank = 1;
        while self.nth_is(n + rank, Punct::Comma) {
            rank += 1;
        }
        self.nth_is(n + rank, Punct::RBracket).then_some(rank)
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
        if self.nth_word(n, &r#in::WORD) || self.nth_word(n, &r#ref::WORD) {
            let at = n + 1 + usize::from(matches!(self.nth(n + 1), Tok::Lifetime(_)));
            return self.type_one_len(at);
        }
        let mut end = match self.nth(n) {
            Tok::Lifetime(_) => return Some(n + 1),
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
        loop {
            if self.nth_is(end, Punct::Question) {
                end += 1;
            } else if let Some(rank) = self.rank_ahead(end) {
                end += rank + 1;
            } else {
                return Some(end);
            }
        }
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
