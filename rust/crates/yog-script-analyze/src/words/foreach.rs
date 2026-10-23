//! `foreach` — once per item.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word, r#in, var};

pub static WORD: Word = Word {
    text: "foreach",
    doc: "Runs a statement once per item: `foreach (var battery in Inventory) Charge.Push(battery.Charge);`, \
          `foreach (var (key, value) in scores) { … }`.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    p.bump();
    p.need(Punct::LParen)?;
    let ty = if p.eat_word(&var::WORD) { None } else { Some(p.ty()?) };
    let binding = var::binding(p)?;
    p.need_word(&r#in::WORD)?;
    let iter = p.braces(Parser::expr)?;
    p.need(Punct::RParen)?;
    let body = Box::new(p.stmt()?);
    Ok(Stmt::Foreach { ty, binding, iter, body })
}
