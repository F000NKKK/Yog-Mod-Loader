//! `before` — code run before a method.

use crate::ast::{PatchItem, Point};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, call, local};

pub static WORD: Word = Word {
    text: "before",
    doc: "Runs before the patched method; inside one, before a call in it (`before call Spawn() { … }`) (`@Inject` at its head): `before bool Hurt(DamageSource source, f32 amount) { … }`. `skip value;` ends the method with that value; falling through runs it.",
    reserved: false,
    roles: &[Role::PatchItem { parse }, Role::Point { parse: point }],
};

fn parse(p: &mut Parser) -> PResult<PatchItem> {
    p.bump();
    let method = p.target()?;
    let uses = local::uses(p)?;
    let body = p.block()?;
    Ok(PatchItem::Before { method, uses, body })
}

/// `before call Callee selector? uses? { … }` inside a method.
fn point(p: &mut Parser) -> PResult<Point> {
    p.bump();
    p.need_word(&call::WORD)?;
    let callee = p.callee()?;
    let select = p.selector()?;
    let uses = local::uses(p)?;
    let body = p.block()?;
    Ok(Point::BeforeCall { callee, select, uses, body })
}
