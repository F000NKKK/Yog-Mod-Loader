//! `foreach` — once per item.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word, r#in, r#let, r#mut};

pub static WORD: Word = Word {
    text: "foreach",
    doc: "Runs a statement once per item: `foreach (let battery in Inventory) Charge.Push(battery.Charge);`, \
          `foreach (let (key, value) in scores) { … }`; `let mut` lends each item to be changed.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    parse_in(p, false)
}

/// `await foreach (…)`: at `await`, `foreach` after it.
pub(crate) fn awaited(p: &mut Parser) -> PResult<Stmt> {
    p.bump();
    parse_in(p, true)
}

/// `foreach (…) body`, its items awaited when `awaits`.
fn parse_in(p: &mut Parser, awaits: bool) -> PResult<Stmt> {
    p.bump();
    p.need(Punct::LParen)?;
    let ty = if p.eat_word(&r#let::WORD) { None } else { Some(p.ty()?) };
    let mutable = p.eat_word(&r#mut::WORD);
    let binding = r#let::binding(p)?;
    p.need_word(&r#in::WORD)?;
    let iter = p.braces(Parser::expr)?;
    p.need(Punct::RParen)?;
    let body = Box::new(p.stmt()?);
    Ok(Stmt::Foreach {
        ty,
        binding,
        iter,
        body,
        mutable,
        awaits,
    })
}
