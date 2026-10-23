//! `try`.

use crate::ast::{Expr, UnOp};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "try",
    doc: "Takes a result's value, or ends this method with its failure: `var v = try Parse(text);`.",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<Expr> {
    p.prefixed(UnOp::Try)
}
