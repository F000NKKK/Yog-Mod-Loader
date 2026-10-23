//! `skip` — the patched method ends here.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "skip",
    doc: "In a patch's `before`: the patched method doesn't run and ends with this value: `skip false`. \
          Falling through runs it.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    let span = p.bump().span;
    Ok(Stmt::Skip(p.optional_value()?, span))
}
