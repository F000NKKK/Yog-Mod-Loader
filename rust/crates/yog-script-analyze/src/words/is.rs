//! `is` — whether a value matches a pattern.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::words::{Level, Role, Word};

pub static WORD: Word = Word {
    text: "is",
    doc: "Whether a value matches a pattern: `e is Monster`, `target is Player p && p.IsAlive`, \
          `x is not null`, `level is > 3 and < 10`. A name binds the narrowed value.",
    reserved: true,
    roles: &[Role::Infix { level: Level::Compare, parse }],
};

fn parse(p: &mut Parser, left: Expr) -> PResult<Expr> {
    p.bump();
    let pattern = p.pattern()?;
    let span = left.span.to(p.prev());
    Ok(Expr { kind: ExprKind::Is { expr: Box::new(left), pattern: Box::new(pattern) }, span })
}
