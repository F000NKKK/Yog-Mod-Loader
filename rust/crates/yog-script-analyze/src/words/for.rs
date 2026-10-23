//! `for` — a counted loop.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word, r#let};

pub static WORD: Word = Word {
    text: "for",
    doc: "A counted loop: `for (let mut i = 0; i < 4; i += 1) { … }`. Over items, `foreach`.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    p.bump();
    p.need(Punct::LParen)?;
    let init = if p.at(Punct::Semi) {
        None
    } else if p.at_word(&r#let::WORD) {
        Some(Box::new(r#let::local(p)?))
    } else if p.typed_ahead().is_some() {
        Some(Box::new(p.typed_local()?))
    } else {
        let e = p.expr()?;
        Some(Box::new(p.assign_or(e)?))
    };
    p.need(Punct::Semi)?;
    let cond = if p.at(Punct::Semi) { None } else { Some(p.expr()?) };
    p.need(Punct::Semi)?;
    let step = if p.at(Punct::RParen) {
        None
    } else {
        let e = p.expr()?;
        Some(Box::new(p.assign_or(e)?))
    };
    p.need(Punct::RParen)?;
    let body = Box::new(p.stmt()?);
    Ok(Stmt::For { init, cond, step, body })
}
