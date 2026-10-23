//! `nameof` — a name as a string.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "nameof",
    doc: "A name as a string, checked: `nameof(Charge.Energy)` is `\"Energy\"`.",
    reserved: false,
    roles: &[Role::Prefix { starts: Some(starts), parse }],
};

fn starts(p: &Parser) -> bool {
    p.at_word(&WORD) && p.nth_is(1, Punct::LParen)
}

fn parse(p: &mut Parser) -> PResult<Expr> {
    let start = p.bump().span;
    p.need(Punct::LParen)?;
    let expr = p.expr()?;
    p.need(Punct::RParen)?;
    Ok(Expr { kind: ExprKind::NameOf(Box::new(expr)), span: start.to(p.prev()) })
}
