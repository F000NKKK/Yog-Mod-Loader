//! `unit` — a mod's own unit of measure.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "unit",
    doc: "A unit of measure of the mod's own: `unit Energy`, then `100Energy`. `unit FE = Energy * 1` \
          converts; different quantities never mix (`Energy + Blocks` is an error).",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("a unit's name")?;
    let value = if p.eat(Punct::Eq) { Some(p.expr()?) } else { None };
    Ok(DeclKind::Unit { name, value })
}
