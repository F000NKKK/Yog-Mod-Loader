//! `typeof` — a type as a value.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "typeof",
    doc: "A type as a `System.Type`: `typeof(Charger)`, `typeof(List<i32>)`; its members, methods \
          and attributes are `System.Reflection`'s.",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<Expr> {
    let start = p.bump().span;
    p.need(Punct::LParen)?;
    let ty = p.ty()?;
    p.need(Punct::RParen)?;
    Ok(Expr { kind: ExprKind::TypeOf(ty), span: start.to(p.prev()) })
}
