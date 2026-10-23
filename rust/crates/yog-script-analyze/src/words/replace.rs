//! `replace` — a method's whole body.

use crate::ast::PatchItem;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "replace",
    doc: "The patched method's whole body (`@Overwrite`): `replace bool CanEat(bool ignoreHunger) { … }`. \
          Exclusive: a second mod replacing it is a conflict reported at launch with both mods' names.",
    reserved: false,
    roles: &[Role::PatchItem { parse }],
};

fn parse(p: &mut Parser) -> PResult<PatchItem> {
    p.bump();
    let method = p.target()?;
    let body = p.block()?;
    Ok(PatchItem::Replace { method, body })
}
