//! `priority` — the order among mods on one point.

use crate::ast::PatchItem;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "priority",
    doc: "This patch's order among mods patching the same points: `priority 100` (higher runs first; \
          `around`s nest in this order).",
    reserved: false,
    roles: &[Role::PatchItem { parse }],
};

fn parse(p: &mut Parser) -> PResult<PatchItem> {
    let span = p.bump().span;
    Ok(PatchItem::Priority(p.int("a priority")?, span))
}
