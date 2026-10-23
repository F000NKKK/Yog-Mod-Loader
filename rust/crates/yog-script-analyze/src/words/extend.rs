//! `extend` — members added to a type, the game's too.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "extend",
    doc: "Adds members to a type, the game's included, where this file is used: \
          `extend Player { val isCharged: Bool => … }`; `extend T : Interface { … }` makes it one.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let ty = p.ty_plain()?;
    let adds = if p.eat(Punct::Colon) { Some(p.ty_plain()?) } else { None };
    Ok(DeclKind::Extend { ty, adds, body: p.body()? })
}
