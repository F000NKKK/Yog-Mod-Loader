//! `add` — members and interfaces added to a patched class.

use crate::ast::PatchItem;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "add",
    doc: "Adds to the patched class: state (`add saved f32 Shield = 0;`), methods (`add void Zap() { … }`), \
          interfaces (`add : IChargeable;`).",
    reserved: false,
    roles: &[Role::PatchItem { parse }],
};

fn parse(p: &mut Parser) -> PResult<PatchItem> {
    p.bump();
    if p.at(Punct::Colon) {
        let bases = p.bases()?;
        p.end()?;
        return Ok(PatchItem::AddBases(bases));
    }
    Ok(PatchItem::Add(Box::new(p.decl()?)))
}
