//! `read` — a field's reads.

use crate::ast::PatchItem;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "read",
    doc: "Every read of a patched class's field answered here: `read foodData { … proceed() }`; \
          `proceed()` is the field as it is. Inside a method, `at read field { … }` runs at a read.",
    reserved: false,
    roles: &[Role::PatchItem { parse }],
};

fn parse(p: &mut Parser) -> PResult<PatchItem> {
    p.bump();
    let field = p.name("a field")?;
    Ok(PatchItem::Read { field, body: p.block()? })
}
