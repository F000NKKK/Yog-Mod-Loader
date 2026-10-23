//! `around` — a wrap of a call inside the method.

use crate::ast::Point;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, call};

pub static WORD: Word = Word {
    text: "around",
    doc: "Wraps a call inside the patched method (`@WrapOperation`): `around call AiStep() { if (!Level.IsFrozen) proceed(); }`. \
          `proceed(…)` is the call as it was; several mods' wraps chain by priority.",
    reserved: false,
    roles: &[Role::Point { parse }],
};

fn parse(p: &mut Parser) -> PResult<Point> {
    p.bump();
    p.need_word(&call::WORD)?;
    let callee = p.callee()?;
    let select = p.selector()?;
    let body = p.value_block()?;
    Ok(Point::Around { callee, select, body })
}
