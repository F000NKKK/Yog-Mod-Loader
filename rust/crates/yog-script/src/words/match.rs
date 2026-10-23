//! `match` — a value by its shape.

use crate::ast::{Arm, Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::source::token::{Punct, Tok};
use crate::words::{Role, Word, r#if};

pub static WORD: Word = Word {
    text: "match",
    doc: "Picks an arm by the value's shape: `match mode { Idle -> 0  Charging(rate) -> rate  else -> 1 }`. \
          Over a union or enum every case must be covered (or `else`). An arm may add `if guard`.",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<Expr> {
    let start = p.bump().span;
    let expr = Box::new(p.expr_no_brace()?);
    p.need(Punct::LBrace)?;
    let mut arms = Vec::new();
    loop {
        p.skip_newlines();
        if p.eat(Punct::RBrace) {
            return Ok(Expr { kind: ExprKind::Match { expr, arms }, span: start.to(p.prev()) });
        }
        if *p.tok() == Tok::Eof {
            return Err(p.expected("`}`"));
        }
        match arm(p) {
            Ok(a) => arms.push(a),
            Err(_) => p.recover(),
        }
        p.eat(Punct::Comma);
    }
}

fn arm(p: &mut Parser) -> PResult<Arm> {
    let pattern = p.pattern()?;
    let guard = if p.eat_word(&r#if::WORD) { Some(p.expr_no_brace()?) } else { None };
    p.need(Punct::Arrow)?;
    let body = if p.at(Punct::LBrace) {
        let block = p.block()?;
        let span = block.span;
        Expr { kind: ExprKind::Block(block), span }
    } else {
        p.expr()?
    };
    Ok(Arm { pattern, guard, body })
}
