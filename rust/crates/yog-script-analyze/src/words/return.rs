//! `return`.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "return",
    doc: "Ends the method, with a value when it gives one: `return Left <= 0t;`.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    let at = p.bump().span;
    let value = p.optional_value()?;
    p.end()?;
    Ok(Stmt::Return(value, at))
}
