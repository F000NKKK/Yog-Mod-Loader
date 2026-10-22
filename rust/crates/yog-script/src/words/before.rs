//! `before` — before a patched method, or a call in it.

use crate::ast::{PatchItem, Point};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, call, uses};

pub static WORD: Word = Word {
    text: "before",
    doc: "Runs before the patched method: `before hurt(source: DamageSource, amount: Float) -> Bool { … }`. `skip value` ends the method there; falling through runs it. Inside a method, `before call X { … }` runs before that call.",
    reserved: false,
    roles: &[Role::PatchItem { parse: item }, Role::Point { parse: point }],
};

fn item(p: &mut Parser) -> PResult<PatchItem> {
    p.bump();
    let method = p.method()?;
    let uses = uses::list(p)?;
    Ok(PatchItem::Before { method, uses, body: p.block()? })
}

/// Inside a method: `before call X …`.
fn point(p: &mut Parser) -> PResult<Point> {
    p.bump();
    p.need_word(&call::WORD)?;
    let callee = p.callee()?;
    let select = p.selector()?;
    let uses = uses::list(p)?;
    Ok(Point::BeforeCall { callee, select, uses, body: p.block()? })
}
