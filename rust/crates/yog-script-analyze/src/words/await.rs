//! `await`.

use crate::ast::{Expr, UnOp};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "await",
    doc: "Waits for a task in an `async` method, the game going on meanwhile: `await Task.Delay(5s);`.",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<Expr> {
    p.prefixed(UnOp::Await)
}
