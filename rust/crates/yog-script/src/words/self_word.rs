//! `self`.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "self",
    doc: "The object the code belongs to: the block, the item, the patched object. Its members are reachable without it.",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<Expr> {
    let span = p.bump().span;
    Ok(Expr { kind: ExprKind::SelfValue, span })
}
