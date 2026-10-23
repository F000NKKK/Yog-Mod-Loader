//! `in` — membership; what `foreach` walks; a declaration's bounds.

use crate::ast::{BinOp, Expr};
use crate::parser::{PResult, Parser};
use crate::words::{Level, Role, Word};

pub static WORD: Word = Word {
    text: "in",
    doc: "Whether a value is among others: `item in #minecraft:logs`, `x in 0..10`. Also what `foreach` \
          walks and a field's bounds (`saved i32 Level in 0..100 = 0;`).",
    reserved: true,
    roles: &[Role::Infix { level: Level::Compare, parse }],
};

fn parse(p: &mut Parser, left: Expr) -> PResult<Expr> {
    p.bump();
    let right = p.binary(Level::Compare.tighter())?;
    Ok(Parser::binary_node(BinOp::In, left, right))
}
