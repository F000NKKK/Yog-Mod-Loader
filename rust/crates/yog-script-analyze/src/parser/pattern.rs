//! Patterns (`ys-grammar.md` §3.3): `switch` arms and `is`.

use super::{PResult, Parser};
use crate::ast::{BinOp, Name, Pattern};
use crate::source::token::{Punct, Tok};
use crate::words::{and, r#let, not, or};

pub(crate) const WILDCARD: &str = "_";

const RELATIONAL: &[(Punct, BinOp)] = &[
    (Punct::Lt, BinOp::Lt),
    (Punct::LtEq, BinOp::LtEq),
    (Punct::Gt, BinOp::Gt),
    (Punct::GtEq, BinOp::GtEq),
];

impl Parser<'_, '_, '_> {
    /// A pattern: `a or b`, `a and b`, `not a`, and the single ones.
    pub(crate) fn pattern(&mut self) -> PResult<Pattern> {
        let mut left = self.pattern_and()?;
        while self.eat_word(&or::WORD) {
            left = Pattern::Or(Box::new(left), Box::new(self.pattern_and()?));
        }
        Ok(left)
    }

    fn pattern_and(&mut self) -> PResult<Pattern> {
        let mut left = self.pattern_not()?;
        while self.eat_word(&and::WORD) {
            left = Pattern::And(Box::new(left), Box::new(self.pattern_not()?));
        }
        Ok(left)
    }

    fn pattern_not(&mut self) -> PResult<Pattern> {
        if self.eat_word(&not::WORD) {
            return Ok(Pattern::Not(Box::new(self.pattern_not()?)));
        }
        self.pattern_one()
    }

    fn pattern_one(&mut self) -> PResult<Pattern> {
        if self.eat_word(&r#let::WORD) {
            return Ok(Pattern::Let(self.name("a name")?));
        }
        if let Some(&(p, op)) = RELATIONAL.iter().find(|(p, _)| self.at(*p)) {
            self.need(p)?;
            return Ok(Pattern::Relational {
                op,
                value: self.binary(crate::words::Level::Sum)?,
            });
        }
        match self.tok() {
            Tok::Ident(name) if name == WILDCARD => Ok(Pattern::Wild(self.bump().span)),
            Tok::Punct(Punct::LParen) => {
                self.bump();
                Ok(Pattern::Tuple(self.list(Punct::RParen, Self::pattern)?))
            }
            Tok::Punct(Punct::LBrace) => self.property_pattern(None),
            Tok::Ident(_) => self.path_pattern(),
            Tok::Keyword(w) if w.is_type() => self.path_pattern(),
            _ => self.value_pattern(),
        }
    }

    /// `Mode.Charging(let rate)`, `Monster m`, `Monster { … }`, or a path
    /// to a value, a case or a type (`Mode.Idle`, `Monster`).
    fn path_pattern(&mut self) -> PResult<Pattern> {
        let start = self.span();
        let written = self.ty()?;
        let ty = self.generic_case(written)?;
        if self.at(Punct::LParen) {
            let path = self.type_path(&ty, start)?;
            self.bump();
            let args = self.list(Punct::RParen, Self::pattern)?;
            return Ok(Pattern::Variant { path, args });
        }
        if self.at(Punct::LBrace) {
            return self.property_pattern(Some(ty));
        }
        if matches!(self.tok(), Tok::Ident(_)) {
            let bind = Some(self.name("a name")?);
            return Ok(Pattern::Type { ty, bind });
        }
        Ok(Pattern::Type { ty, bind: None })
    }

    /// `Maybe<i32>.Just`: a generic enum's case, its arguments dropped —
    /// what's matched gives them.
    fn generic_case(&mut self, ty: crate::ast::Type) -> PResult<crate::ast::Type> {
        let crate::ast::TypeKind::Path { path, args } = &ty.kind else {
            return Ok(ty);
        };
        if args.is_empty() || !self.at(Punct::Dot) {
            return Ok(ty);
        }
        let mut path = path.clone();
        while self.eat(Punct::Dot) {
            path.push(self.name("a case")?);
        }
        Ok(crate::ast::Type {
            kind: crate::ast::TypeKind::Path {
                path,
                args: Vec::new(),
            },
            span: ty.span.to(self.prev()),
        })
    }

    /// `{ Name: pattern, … }`, after its type if any.
    fn property_pattern(&mut self, ty: Option<crate::ast::Type>) -> PResult<Pattern> {
        self.need(Punct::LBrace)?;
        let fields = self.list(Punct::RBrace, |p| {
            let name: Name = p.name("a member")?;
            p.need(Punct::Colon)?;
            Ok((name, p.pattern()?))
        })?;
        Ok(Pattern::Property { ty, fields })
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

    /// A variant's path: the type written as a plain path.
    fn type_path(
        &mut self,
        ty: &crate::ast::Type,
        at: crate::source::token::Span,
    ) -> PResult<Vec<Name>> {
        match &ty.kind {
            crate::ast::TypeKind::Path { path, args } if args.is_empty() => Ok(path.clone()),
            _ => Err(self.error(
                yog_ids::script_problem::EXPECTED,
                at,
                "a case is named by a path: `Mode.Charging(let rate)`",
            )),
        }
    }
}
