//! `enum` — one of several cases, each with its own data.

use crate::ast::{DeclKind, Variant};
use crate::parser::{PResult, Parser};
use crate::source::token::{Punct, Tok};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "enum",
    doc: "One of several cases, as Rust's: `enum Mode { Idle, Charging(i32 Rate), Full }` \
          (`Mode.Charging(4)`), or numbered, as C#'s: `enum Tier : u8 { Wood = 1, Stone, Iron }`. \
          Members follow the cases after `;`. A `switch` over it covers every case.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("an enum's name")?;
    let generics = p.generics()?;
    let base = if p.eat(Punct::Colon) { Some(p.ty()?) } else { None };
    p.need(Punct::LBrace)?;
    let mut variants = Vec::new();
    while !p.at(Punct::RBrace) && !p.at(Punct::Semi) {
        variants.push(variant(p)?);
        if !p.eat(Punct::Comma) {
            break;
        }
    }
    let body = if p.eat(Punct::Semi) {
        p.decls_to_brace()?
    } else {
        p.need(Punct::RBrace)?;
        Vec::new()
    };
    Ok(DeclKind::Enum { name, generics, base, variants, body })
}

/// `Idle`, `Charging(i32 Rate)`, `Wood = 1`, its `///` lines before it.
fn variant(p: &mut Parser) -> PResult<Variant> {
    let mut doc = Vec::new();
    while let Tok::Doc(line) = p.tok() {
        doc.push(line.clone());
        p.bump();
    }
    let name = p.name("a case")?;
    let fields = if p.at(Punct::LParen) { p.params()? } else { Vec::new() };
    let value = if p.eat(Punct::Eq) { Some(p.expr()?) } else { None };
    Ok(Variant { doc, name, fields, value })
}
