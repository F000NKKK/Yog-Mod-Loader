//! `while` — again while a condition holds.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "while",
    doc: "Runs a statement again while a condition holds: `while (queue.Count > 0) { … }`.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    p.bump();
    let cond = p.paren_cond()?;
    let body = Box::new(p.stmt()?);
    Ok(Stmt::While { cond, body })
}
