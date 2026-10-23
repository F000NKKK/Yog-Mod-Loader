//! `null`.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "null",
    doc: "No value: only a nullable type (`Player?`) can be `null`.",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<Expr> {
    let span = p.bump().span;
    Ok(Expr { kind: ExprKind::Null, span })
}
