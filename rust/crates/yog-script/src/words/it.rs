//! `it`.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "it",
    doc: "A lambda's one parameter when it names none: `players.where { it.level > 10 }`.",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<Expr> {
    let span = p.bump().span;
    Ok(Expr { kind: ExprKind::It, span })
}
