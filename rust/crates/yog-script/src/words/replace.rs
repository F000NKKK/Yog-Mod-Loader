//! `replace` — a patched method's whole body.

use crate::ast::PatchItem;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "replace",
    doc: "Replaces a patched method's whole body: `replace canEat(ignoreHunger: Bool) -> Bool { … }`. \
          Exclusive: two mods replacing one method is a conflict named at launch with both mods.",
    reserved: false,
    roles: &[Role::PatchItem { parse }],
};

fn parse(p: &mut Parser) -> PResult<PatchItem> {
    p.bump();
    let method = p.method()?;
    Ok(PatchItem::Replace { method, body: p.block()? })
}
