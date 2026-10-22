//! `after` — after a patched method, or a call in it.

use crate::ast::{PatchItem, Point};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, call, uses};

pub static WORD: Word = Word {
    text: "after",
    doc: "Runs after the patched method; `result` is its value and can be changed: `after getMaxHealth() -> Float { result += shield / 2 }`. Inside a method, `after call X { … }` runs after that call.",
    reserved: false,
    roles: &[Role::PatchItem { parse: item }, Role::Point { parse: point }],
};

fn item(p: &mut Parser) -> PResult<PatchItem> {
    p.bump();
    let method = p.method()?;
    let uses = uses::list(p)?;
    Ok(PatchItem::After { method, uses, body: p.block()? })
}

/// Inside a method: `after call X …`.
fn point(p: &mut Parser) -> PResult<Point> {
    p.bump();
    p.need_word(&call::WORD)?;
    let callee = p.callee()?;
    let select = p.selector()?;
    let uses = uses::list(p)?;
    Ok(Point::AfterCall { callee, select, uses, body: p.block()? })
}
