//! `skip` — ends the patched method.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "skip",
    doc: "In a patch's `before`: ends the patched method with a value (`skip false;`); falling through runs it.",
    reserved: false,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    let at = p.bump().span;
    let value = p.optional_value()?;
    p.end()?;
    Ok(Stmt::Skip(value, at))
}
