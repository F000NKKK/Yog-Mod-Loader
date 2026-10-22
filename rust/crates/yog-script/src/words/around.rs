//! `around` — a call inside a patched method, wrapped.

use crate::ast::Point;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, call};

pub static WORD: Word = Word {
    text: "around",
    doc: "Wraps a call inside the patched method: `around call aiStep() { if !level.isFrozen { proceed() } }`. \
          `proceed(…)` is the call as it was; several mods' wraps nest by priority, none fails.",
    reserved: false,
    roles: &[Role::Point { parse }],
};

fn parse(p: &mut Parser) -> PResult<Point> {
    p.bump();
    p.need_word(&call::WORD)?;
    let callee = p.callee()?;
    let select = p.selector()?;
    Ok(Point::Around { callee, select, body: p.block()? })
}
