//! `and`.

use crate::ast::{BinOp, Expr};
use crate::parser::{PResult, Parser};
use crate::words::{Level, Role, Word};

pub static WORD: Word = Word {
    text: "and",
    doc: "Both hold: `a and b` (or `a && b`); `b` is evaluated only when `a` holds.",
    reserved: true,
    roles: &[Role::Infix { level: Level::And, parse, negatable: false }],
};

fn parse(p: &mut Parser, left: Expr, _negated: bool) -> PResult<Expr> {
    p.bump();
    let right = p.binary(Level::And.tighter())?;
    Ok(Parser::binary_node(BinOp::And, left, right))
}
