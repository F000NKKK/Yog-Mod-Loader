//! `move` — ownership handed over.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "move",
    doc: "Hands a value over: `void Keep(move Stream s)`, called `Keep(move stream)`; the caller's \
          `stream` can't be used after; `let b = move a;`. Who owns a value disposes it (`IDisposable`).",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<Expr> {
    let start = p.bump().span;
    let expr = p.unary()?;
    Ok(Expr { kind: ExprKind::Move(Box::new(expr)), span: start.to(p.prev()) })
}
