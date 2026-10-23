//! `while` — again while a condition holds.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, r#if};

pub static WORD: Word = Word {
    text: "while",
    doc: "Runs a block again while a condition holds: `while energy > 0 { … }`, `while let x = next() { … }`. \
          To wait across ticks, use `Task.until { … }` instead.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    p.bump();
    let cond = r#if::cond(p)?;
    Ok(Stmt::While { cond, body: p.block()? })
}
