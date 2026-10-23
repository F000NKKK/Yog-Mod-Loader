//! `slots` — an inventory on content.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word, of};

pub static WORD: Word = Word {
    text: "slots",
    doc: "An inventory on content: `slots inventory[4] of Battery` (only batteries go in). Saved and synced like state.",
    reserved: false,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let name = p.name("the slots' name")?;
    p.need(Punct::LBracket)?;
    let size = p.expr()?;
    p.need(Punct::RBracket)?;
    let of = of::ty(p)?;
    Ok(DeclKind::Slots { name, size, of })
}
