//! `continue`.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "continue",
    doc: "Goes on to the loop's next round.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    let at = p.bump().span;
    p.end()?;
    Ok(Stmt::Continue(at))
}
