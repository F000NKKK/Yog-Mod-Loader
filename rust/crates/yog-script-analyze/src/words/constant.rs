//! `constant` — a constant in the method.

use crate::ast::Point;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "constant",
    doc: "A constant in the patched method, changed (`@ModifyConstant`): `constant 0.4 => 0.2;`.",
    reserved: false,
    roles: &[Role::Point { parse }],
};

fn parse(p: &mut Parser) -> PResult<Point> {
    p.bump();
    let value = p.unary()?;
    p.need(Punct::FatArrow)?;
    let to = p.expr()?;
    let select = p.selector()?;
    p.end()?;
    Ok(Point::Constant { value, to, select })
}
