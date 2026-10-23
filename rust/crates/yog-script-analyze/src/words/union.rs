//! `union` — a value that is one of several cases, each with its own data.

use crate::ast::{DeclKind, Variant};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "union",
    doc: "A value that is one of several cases, each with its own data: \
          `union Mode { Idle, Charging(int Rate), Full }`. A `switch` over it covers every case.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("a union's name")?;
    let generics = p.generics()?;
    p.need(Punct::LBrace)?;
    let mut variants = Vec::new();
    loop {
        let name = p.name("a case")?;
        let fields = if p.at(Punct::LParen) { p.params()? } else { Vec::new() };
        variants.push(Variant { name, fields });
        if !p.eat(Punct::Comma) || p.at(Punct::RBrace) || p.at(Punct::Semi) {
            break;
        }
    }
    let body = if p.eat(Punct::Semi) { p.decls_to_brace()? } else {
        p.need(Punct::RBrace)?;
        Vec::new()
    };
    Ok(DeclKind::Union { name, generics, variants, body })
}
