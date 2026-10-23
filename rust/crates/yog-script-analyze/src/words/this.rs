//! `this`.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "this",
    doc: "The object the code is in: a type's, a content's, a component's members, a patch's (the patched object).",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<Expr> {
    let span = p.bump().span;
    Ok(Expr { kind: ExprKind::This, span })
}
