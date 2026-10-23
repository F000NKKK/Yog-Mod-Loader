//! `switch` — a value by its shape.

use crate::ast::{Arm, Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Level, Role, Word, when};

pub static WORD: Word = Word {
    text: "switch",
    doc: "Picks an arm by the value's shape: `tier switch { Tier.Wood or Tier.Stone => 1, Tier.Iron => 2, _ => 3 }`. \
          Over a union or enum every case must be covered (or `_`); an arm may add `when guard`.",
    reserved: true,
    roles: &[Role::Infix { level: Level::Switch, parse }],
};

fn parse(p: &mut Parser, left: Expr) -> PResult<Expr> {
    p.bump();
    p.need(Punct::LBrace)?;
    let arms = p.braces(|p| p.list(Punct::RBrace, arm))?;
    let span = left.span.to(p.prev());
    Ok(Expr { kind: ExprKind::Switch { expr: Box::new(left), arms }, span })
}

fn arm(p: &mut Parser) -> PResult<Arm> {
    let pattern = p.pattern()?;
    let guard = if p.eat_word(&when::WORD) { Some(p.expr()?) } else { None };
    p.need(Punct::FatArrow)?;
    Ok(Arm { pattern, guard, value: p.expr()? })
}
