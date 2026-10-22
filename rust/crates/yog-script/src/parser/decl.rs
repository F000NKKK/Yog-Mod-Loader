//! Declarations: a file's, a body's — documentation and modifiers taken,
//! then the first word whose form starts here parses it.

use super::{PResult, Parser};
use crate::ast::{Decl, DeclKind, File};
use crate::token::{Punct, Tok};
use crate::words::{ALL, Role};

impl Parser<'_, '_, '_> {
    /// The whole source.
    pub fn file(&mut self) -> File {
        let mut decls = Vec::new();
        loop {
            self.skip_newlines_keeping_docs();
            if *self.tok() == Tok::Eof {
                return File { decls };
            }
            self.decl_into(&mut decls);
        }
    }

    /// `{ decls }`.
    pub(crate) fn body(&mut self) -> PResult<Vec<Decl>> {
        self.need(Punct::LBrace)?;
        let mut decls = Vec::new();
        self.braces(|p| {
            loop {
                p.skip_newlines_keeping_docs();
                if p.eat(Punct::RBrace) {
                    return Ok(decls);
                }
                if *p.tok() == Tok::Eof {
                    return Err(p.expected("`}`"));
                }
                p.decl_into(&mut decls);
            }
        })
    }

    /// One declaration and its end, or past it when it fails.
    fn decl_into(&mut self, decls: &mut Vec<Decl>) {
        match self.decl().and_then(|d| self.end_stmt().map(|()| d)) {
            Ok(d) => decls.push(d),
            Err(_) => {
                self.recover();
                self.eat(Punct::Semi);
            }
        }
    }

    pub(crate) fn decl(&mut self) -> PResult<Decl> {
        let start = self.span();
        let mut doc = Vec::new();
        while let Tok::Doc(line) = self.tok() {
            doc.push(line.clone());
            self.bump();
            self.skip_newlines_keeping_docs();
        }
        let mut modifiers = Vec::new();
        while let Some(m) = self.modifier_here() {
            modifiers.push(m);
            self.bump();
        }
        let kind = self.decl_kind()?;
        Ok(Decl {
            doc,
            modifiers,
            kind,
            span: start.to(self.prev()),
        })
    }

    fn modifier_here(&self) -> Option<crate::ast::Modifier> {
        ALL.iter().filter(|w| self.at_word(w)).find_map(|w| {
            w.role(|r| {
                if let Role::Modifier(m) = r {
                    Some(*m)
                } else {
                    None
                }
            })
        })
    }

    fn decl_kind(&mut self) -> PResult<DeclKind> {
        // A name and `=` is a property, whatever word the name is.
        let named = matches!(self.tok(), Tok::Ident(_));
        if named && matches!(self.nth(1), Tok::Punct(Punct::Eq | Punct::FatArrow)) {
            return self.generic_decl();
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
        self.generic_decl()
    }

    /// Newlines skipped, a doc line kept for the declaration after it.
    fn skip_newlines_keeping_docs(&mut self) {
        while *self.tok() == Tok::Newline {
            self.bump();
        }
    }
}
