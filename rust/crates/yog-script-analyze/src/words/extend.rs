//! `extend` — members, state, interfaces and components added to a type.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "extend",
    doc: "Adds to a type, the game's included: members (`extend Player { public bool IsCharged => …; }`), \
          state (`saved int Visits = 0;`: per player), interfaces (`: IName`) and components \
          (`extend Player has Charge(500);`).",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let ty = p.ty()?;
    let header = p.header(false)?;
    let body = if p.eat(Punct::Semi) { Vec::new() } else { p.body()? };
    Ok(DeclKind::Extend { ty, header, body })
}
