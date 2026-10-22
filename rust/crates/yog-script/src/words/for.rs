//! `for` — once per item.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, r#in};

pub static WORD: Word = Word {
    text: "for",
    doc: "Runs a block once per item: `for b in inventory { … }`, `for (k, v) in map { … }`.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    p.bump();
    let pattern = p.pattern()?;
    p.need_word(&r#in::WORD)?;
    let iter = p.expr_no_brace()?;
    Ok(Stmt::For { pattern, iter, body: p.block()? })
}
