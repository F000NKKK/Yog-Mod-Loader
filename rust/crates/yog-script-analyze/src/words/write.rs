//! `write` — a field's every write.

use crate::ast::PatchItem;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "write",
    doc: "Every write of a field of the patched class: `write ExperienceLevel { if (value >= ExperienceLevel) proceed(value); }` — `value` is the new one, `proceed(v)` writes.",
    reserved: false,
    roles: &[Role::PatchItem { parse }],
};

fn parse(p: &mut Parser) -> PResult<PatchItem> {
    p.bump();
    let field = p.any_name("a field")?;
    let body = p.block()?;
    Ok(PatchItem::Write { field, body })
}
