//! The declarations no word starts: a property (`name = value`,
//! `name => value`) and content of any kind (`block Charger { … }`, a
//! mod's own kinds: the kinds are open).

use super::{PResult, Parser};
use crate::ast::{DeclKind, Prop};
use crate::source::token::{Punct, Tok};

impl Parser<'_, '_, '_> {
    pub(super) fn generic_decl(&mut self) -> PResult<DeclKind> {
        let Tok::Ident(_) = self.tok() else {
            return Err(self.expected("a declaration"));
        };
        match self.nth(1) {
            Tok::Punct(Punct::Eq | Punct::FatArrow) => self.prop().map(DeclKind::Prop),
            Tok::Ident(_) => self.content(),
            _ => Err(self.expected("a declaration")),
        }
    }

    /// `name = value` or `name => value`.
    pub(crate) fn prop(&mut self) -> PResult<Prop> {
        let name = self.name("a property")?;
        let computed = self.eat(Punct::FatArrow);
        if !computed {
            self.need(Punct::Eq)?;
        }
        let value = self.expr()?;
        Ok(Prop {
            name,
            value,
            computed,
        })
    }

    /// `kind Name (: Base)? { body }`.
    fn content(&mut self) -> PResult<DeclKind> {
        let kind = self.name("a content kind")?;
        let name = self.name("a name")?;
        let base = if self.eat(Punct::Colon) {
            Some(self.ty()?)
        } else {
            None
        };
        let body = self.body()?;
        Ok(DeclKind::Content {
            kind,
            name,
            base,
            body,
        })
    }
}
