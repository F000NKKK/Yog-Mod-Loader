//! `goto`.

use crate::ast::{Goto, Stmt};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, case, default};

pub static WORD: Word = Word {
    text: "goto",
    doc: "Goes on from a label of its block or one around it (`goto retry;` to `retry:`), or, in a `switch`, \
          from another section: `goto case 2;`, `goto default;` (C#'s).",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    let start = p.bump().span;
    let to = if p.at_word(&case::WORD) {
        p.bump();
        Goto::Case(p.expr()?)
    } else if p.at_word(&default::WORD) {
        p.bump();
        Goto::Default
    } else {
        Goto::Label(p.name("a label")?)
    };
    let at = start.to(p.prev());
    p.end()?;
    Ok(Stmt::Goto(to, at))
}
