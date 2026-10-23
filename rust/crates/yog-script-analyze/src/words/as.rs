//! `as` — a value as another type, or `null`.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::words::{Level, Role, Word};

pub static WORD: Word = Word {
    text: "as",
    doc: "A value as another type, `null` when it isn't one: `(this as IChargeable)?.Shield`. A cast \
          that must hold is `(T)x`; lossy conversions are calls (`Math.Floor(x)`).",
    reserved: true,
    roles: &[Role::Infix { level: Level::Compare, parse }],
};

fn parse(p: &mut Parser, left: Expr) -> PResult<Expr> {
    p.bump();
    let ty = p.ty()?;
    let span = left.span.to(p.prev());
    Ok(Expr { kind: ExprKind::As { expr: Box::new(left), ty }, span })
}
