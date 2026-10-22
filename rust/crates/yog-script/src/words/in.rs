//! `in` — membership; also what `for` walks and a type's bounds.

use crate::ast::{BinOp, Expr};
use crate::parser::{PResult, Parser};
use crate::words::{Level, Role, Word};

pub static WORD: Word = Word {
    text: "in",
    doc: "Whether a value is among others: `item in #minecraft:logs`, `x !in 0..10`. \
          Also what `for` walks (`for b in inventory`) and a type's bounds (`Int in 0..100`).",
    reserved: true,
    roles: &[Role::Infix { level: Level::Compare, parse, negatable: true }],
};

fn parse(p: &mut Parser, left: Expr, negated: bool) -> PResult<Expr> {
    p.bump();
    let right = p.binary(Level::Compare.tighter())?;
    let op = if negated { BinOp::NotIn } else { BinOp::In };
    Ok(Parser::binary_node(op, left, right))
}
