//! `write` — a field's writes.

use crate::ast::PatchItem;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "write",
    doc: "Every write of a patched class's field passes here: `write experienceLevel(value) { if … { proceed(value) } }`; \
          not calling `proceed` drops the write. Inside a method, `at write field { … }` runs at a write.",
    reserved: false,
    roles: &[Role::PatchItem { parse }],
};

fn parse(p: &mut Parser) -> PResult<PatchItem> {
    p.bump();
    let field = p.name("a field")?;
    p.need(Punct::LParen)?;
    let value = p.name("the written value")?;
    p.need(Punct::RParen)?;
    Ok(PatchItem::Write { field, value, body: p.block()? })
}
