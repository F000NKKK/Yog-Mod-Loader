//! `if` — one branch or another.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, r#else};

pub static WORD: Word = Word {
    text: "if",
    doc: "Runs a statement when a condition holds, else another: `if (Level.IsDay) Charge(); else { … }`. \
          A value is `c ? a : b`.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    p.bump();
    let cond = p.paren_cond()?;
    let then = Box::new(p.stmt()?);
    let otherwise = if p.eat_word(&r#else::WORD) { Some(Box::new(p.stmt()?)) } else { None };
    Ok(Stmt::If { cond, then, otherwise })
}
