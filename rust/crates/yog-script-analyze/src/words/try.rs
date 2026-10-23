//! `try`.

use crate::ast::{Expr, ExprKind, UnOp};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "try",
    doc: "Takes a result's value, or ends this function with its failure: `let v = try parse(text)`.",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<Expr> {
    let start = p.bump().span;
    let expr = p.unary()?;
    let span = start.to(expr.span);
    Ok(Expr { kind: ExprKind::Unary { op: UnOp::Try, expr: Box::new(expr) }, span })
}
