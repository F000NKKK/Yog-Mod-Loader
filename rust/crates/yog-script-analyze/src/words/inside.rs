//! `inside` — points within a patched method.

use crate::ast::PatchItem;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "inside",
    doc: "Points within a patched method: `inside tick() { around call aiStep() { … } }`. \
          A point matching several places needs `first`, `last` or `where`.",
    reserved: false,
    roles: &[Role::PatchItem { parse }],
};

fn parse(p: &mut Parser) -> PResult<PatchItem> {
    p.bump();
    let method = p.method()?;
    Ok(PatchItem::Inside { method, points: p.points()? })
}
