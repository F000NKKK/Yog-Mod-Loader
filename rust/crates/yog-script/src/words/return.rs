//! `return` — the function ends with a value.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "return",
    doc: "Ends the function, with a value or none: `return`, `return total`.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    let span = p.bump().span;
    Ok(Stmt::Return(p.optional_value()?, span))
}
