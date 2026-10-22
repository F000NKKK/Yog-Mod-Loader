//! `if` — one branch or another.

use crate::ast::{Cond, Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::token::Punct;
use crate::words::{Role, Word, r#else, r#let};

pub static WORD: Word = Word {
    text: "if",
    doc: "Runs a block when a condition holds, else another: `if level.isDay { … } else { … }`. \
          A value too: `let tier = if x > 3 { 2 } else { 1 }`. `if let p = target { … }` runs when \
          `target` isn't `none`. Conditions take no parentheses; blocks always take braces.",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

pub(crate) fn parse(p: &mut Parser) -> PResult<Expr> {
    let start = p.bump().span;
    let cond = Box::new(cond(p)?);
    let then = p.block()?;
    let otherwise = r#else::branch(p)?;
    Ok(Expr { kind: ExprKind::If { cond, then, otherwise }, span: start.to(p.prev()) })
}

/// A condition: `expr` or `let pattern = expr` (`{` ends it).
pub(crate) fn cond(p: &mut Parser) -> PResult<Cond> {
    if !p.eat_word(&r#let::WORD) {
        return p.expr_no_brace().map(Cond::Expr);
    }
    let pattern = p.pattern()?;
    p.need(Punct::Eq)?;
    let value = p.expr_no_brace()?;
    Ok(Cond::Let { pattern: Box::new(pattern), value })
}
