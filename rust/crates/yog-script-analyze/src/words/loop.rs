//! `loop` — again until `break`.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "loop",
    doc: "Runs a block again until a `break`: `loop { … if done { break } }`.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    p.bump();
    Ok(Stmt::Loop(p.block()?))
}
