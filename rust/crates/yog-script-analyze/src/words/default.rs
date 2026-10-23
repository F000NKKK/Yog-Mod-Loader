//! `default` — a type's zero value.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "default",
    doc: "A type's zero value: `default(i32)` is `0`, `default(Player?)` `null`; `default` alone takes \
          the type wanted where it stands.",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<Expr> {
    let start = p.bump().span;
    let ty = if p.eat(Punct::LParen) {
        let ty = p.ty()?;
        p.need(Punct::RParen)?;
        Some(ty)
    } else {
        None
    };
    Ok(Expr { kind: ExprKind::Default(ty), span: start.to(p.prev()) })
}
