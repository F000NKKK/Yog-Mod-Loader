//! `or`.

use crate::ast::{BinOp, Expr};
use crate::parser::{PResult, Parser};
use crate::words::{Level, Role, Word};

pub static WORD: Word = Word {
    text: "or",
    doc: "Either holds: `a or b` (or `a || b`); `b` is evaluated only when `a` doesn't.",
    reserved: true,
    roles: &[Role::Infix { level: Level::Or, parse, negatable: false }],
};

fn parse(p: &mut Parser, left: Expr, _negated: bool) -> PResult<Expr> {
    p.bump();
    let right = p.binary(Level::Or.tighter())?;
    Ok(Parser::binary_node(BinOp::Or, left, right))
}
