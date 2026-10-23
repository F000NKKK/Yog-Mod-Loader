//! `after` — code run after a method.

use crate::ast::{PatchItem, Point};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, call, local};

pub static WORD: Word = Word {
    text: "after",
    doc: "Runs after the patched method (inside one, after a call in it: `after call Spawn() { … }`): `after f32 GetMaxHealth() { result += Shield / 2; }` — `result` is its value, assignable.",
    reserved: false,
    roles: &[Role::PatchItem { parse }, Role::Point { parse: point }],
};

fn parse(p: &mut Parser) -> PResult<PatchItem> {
    p.bump();
    let method = p.target()?;
    let uses = local::uses(p)?;
    let body = p.block()?;
    Ok(PatchItem::After { method, uses, body })
}

/// `after call Callee selector? uses? { … }` inside a method.
fn point(p: &mut Parser) -> PResult<Point> {
    p.bump();
    p.need_word(&call::WORD)?;
    let callee = p.callee()?;
    let select = p.selector()?;
    let uses = local::uses(p)?;
    let body = p.block()?;
    Ok(Point::AfterCall { callee, select, uses, body })
}
