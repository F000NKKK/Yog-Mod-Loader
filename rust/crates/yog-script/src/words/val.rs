//! `val` — a value computed on every read.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "val",
    doc: "A value computed each time it's read: `val room => capacity - charge`. In a screen, a live binding.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("a value")?;
    let ty = if p.eat(Punct::Colon) { Some(p.ty()?) } else { None };
    p.need(Punct::FatArrow)?;
    Ok(DeclKind::Val { name, ty, value: p.expr()? })
}
