//! `constant` — a constant in a patched method, changed.

use crate::ast::Point;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "constant",
    doc: "A constant written in the patched method, changed: `constant 0.4 -> 0.2`; add `first`, `last` \
          or `where` when it's written more than once.",
    reserved: false,
    roles: &[Role::Point { parse }],
};

fn parse(p: &mut Parser) -> PResult<Point> {
    p.bump();
    let value = p.unary()?;
    p.need(Punct::Arrow)?;
    let to = p.expr()?;
    let select = p.selector()?;
    Ok(Point::Constant { value, to, select })
}
