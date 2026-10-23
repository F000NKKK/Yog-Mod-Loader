//! Declarations: a file's, a body's — attributes, documentation and
//! modifiers taken, then the first word whose form starts here parses it,
//! else a member written type first.

use super::{PResult, Parser};
use crate::ast::{Attribute, Decl, DeclKind, File, Modifier};
use crate::source::token::{Punct, Tok};
use crate::words::{ALL, Role};

impl Parser<'_, '_, '_> {
    /// The whole source.
    pub fn file(&mut self) -> File {
        let mut decls = Vec::new();
        while *self.tok() != Tok::Eof {
            self.decl_into(&mut decls);
        }
        File { decls }
    }

    /// `{ decls }`.
    pub(crate) fn body(&mut self) -> PResult<Vec<Decl>> {
        self.need(Punct::LBrace)?;
        self.braces(Self::decls_to_brace)
    }

    /// Declarations up to and with the `}` closing them.
    pub(crate) fn decls_to_brace(&mut self) -> PResult<Vec<Decl>> {
        let mut decls = Vec::new();
        loop {
            if self.eat(Punct::RBrace) {
                return Ok(decls);
            }
            if *self.tok() == Tok::Eof {
                return Err(self.expected("`}`"));
            }
            self.decl_into(&mut decls);
        }
    }

    /// One declaration, or past it when it fails.
    fn decl_into(&mut self, decls: &mut Vec<Decl>) {
        match self.decl() {
            Ok(d) => decls.push(d),
            Err(_) => self.recover(),
        }
    }

    pub(crate) fn decl(&mut self) -> PResult<Decl> {
        let start = self.span();
        let mut doc = Vec::new();
        let mut attributes = Vec::new();
        loop {
            match self.tok() {
                Tok::Doc(line) => {
                    doc.push(line.clone());
                    self.bump();
                }
                Tok::Punct(Punct::LBracket) => attributes.extend(self.attributes()?),
                _ => break,
            }
        }
        let mut modifiers = Vec::new();
        while let Some(m) = self.modifier_here() {
            modifiers.push(m);
            self.bump();
        }
        let kind = self.decl_kind()?;
        Ok(Decl {
            attributes,
            doc,
            modifiers,
            kind,
            span: start.to(self.prev()),
        })
    }

    /// `[A, B(…)]`.
    fn attributes(&mut self) -> PResult<Vec<Attribute>> {
        self.need(Punct::LBracket)?;
        self.list(Punct::RBracket, |p| {
            let path = p.path("an attribute")?;
            let args = if p.at(Punct::LParen) {
                p.args()?
            } else {
                Vec::new()
            };
            Ok(Attribute { path, args })
        })
    }

    fn modifier_here(&self) -> Option<Modifier> {
        ALL.iter().filter(|w| self.at_word(w)).find_map(|w| {
            w.role(|r| match r {
                Role::Modifier(m) => Some(*m),
                _ => None,
            })
        })
    }

    fn decl_kind(&mut self) -> PResult<DeclKind> {
        // `Name = value;` is a property, whatever word the name is.
        if matches!(self.tok(), Tok::Ident(_)) && self.nth_is(1, Punct::Eq) {
            return self.prop().map(DeclKind::Prop);
        }
        for word in ALL {
            for role in word.roles {
                if let Role::Decl { starts, parse } = role
                    && self.starts(word, *starts)
                {
                    return parse(self);
                }
            }
        }
        self.member()
    }
}
