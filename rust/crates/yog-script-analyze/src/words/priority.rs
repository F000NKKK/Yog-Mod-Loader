//! `priority` — the order of patches on one point.

use crate::ast::PatchItem;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "priority",
    doc: "Where this patch stands among several on one point: `priority 100;` (higher wraps outer).",
    reserved: false,
    roles: &[Role::PatchItem { parse }],
};

fn parse(p: &mut Parser) -> PResult<PatchItem> {
    let at = p.bump().span;
    let value = p.int("a priority")?;
    p.end()?;
    Ok(PatchItem::Priority(value, at.to(p.prev())))
}
