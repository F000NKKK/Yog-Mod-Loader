//! `do` — at least once, then while a condition holds.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, r#while};

pub static WORD: Word = Word {
    text: "do",
    doc: "Runs a statement once, then again while a condition holds: `do { … } while (more);`.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    p.bump();
    let body = Box::new(p.stmt()?);
    p.need_word(&r#while::WORD)?;
    let cond = p.paren_cond()?;
    p.end()?;
    Ok(Stmt::DoWhile { body, cond })
}
