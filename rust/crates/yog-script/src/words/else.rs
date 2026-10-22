//! `else` — the other branch.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, r#if};

pub static WORD: Word = Word {
    text: "else",
    doc: "The other branch: of an `if` (`else { … }`, `else if …`), of a `version` block, \
          or a `match` arm that takes what no other does.",
    reserved: true,
    roles: &[Role::Part],
};

/// `else { … }` / `else if …` after an `if`, or nothing.
pub(crate) fn branch(p: &mut Parser) -> PResult<Option<Box<Expr>>> {
    if !p.eat_word(&WORD) {
        return Ok(None);
    }
    if p.at_word(&r#if::WORD) {
        return r#if::parse(p).map(|e| Some(Box::new(e)));
    }
    let block = p.block()?;
    let span = block.span;
    Ok(Some(Box::new(Expr { kind: ExprKind::Block(block), span })))
}
