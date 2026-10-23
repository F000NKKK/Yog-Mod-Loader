//! `has` — the components an object carries.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::words::{Level, Role, Word};

pub static WORD: Word = Word {
    text: "has",
    doc: "The components an object carries: `block Charger has Charge(1000), Owned { … }`; as a test, \
          `x has Charge`.",
    reserved: false,
    roles: &[Role::Infix { level: Level::Compare, parse }, Role::Part],
};

fn parse(p: &mut Parser, left: Expr) -> PResult<Expr> {
    p.bump();
    let ty = p.ty()?;
    let span = left.span.to(p.prev());
    Ok(Expr { kind: ExprKind::Has { expr: Box::new(left), ty }, span })
}
