//! `inside` — points inside a method.

use crate::ast::PatchItem;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "inside",
    doc: "Points inside the patched method: `inside void Tick() { around call AiStep() { … } }` — calls, \
          field reads and writes, `new`s, constants, locals, each by its meaning.",
    reserved: false,
    roles: &[Role::PatchItem { parse }],
};

fn parse(p: &mut Parser) -> PResult<PatchItem> {
    p.bump();
    let method = p.target()?;
    let points = p.points()?;
    Ok(PatchItem::Inside { method, points })
}
