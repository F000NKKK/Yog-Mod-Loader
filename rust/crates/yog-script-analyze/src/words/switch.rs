//! `switch` — a value by its shape.

use crate::ast::{Arm, Expr, ExprKind, Section, Stmt};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Level, Role, Word, case, default, when};

pub static WORD: Word = Word {
    text: "switch",
    doc: "Picks an arm by the value's shape: `tier switch { Tier.Wood or Tier.Stone => 1, Tier.Iron => 2, _ => 3 }`. \
          Over a union or enum every case must be covered (or `_`); an arm may add `when guard`. \
          As a statement, C#'s: `switch (n) { case 1: … break; case 2: case 3: … break; default: … break; }`, \
          a section's `break` leaving the switch.",
    reserved: true,
    roles: &[Role::Infix { level: Level::Switch, parse }, Role::Stmt { parse: statement }],
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

/// `switch (expr) { sections }`.
fn statement(p: &mut Parser) -> PResult<Stmt> {
    p.bump();
    p.need(Punct::LParen)?;
    let expr = p.braces(Parser::expr)?;
    p.need(Punct::RParen)?;
    p.need(Punct::LBrace)?;
    let mut sections = Vec::new();
    while !p.at(Punct::RBrace) {
        sections.push(section(p)?);
    }
    p.need(Punct::RBrace)?;
    Ok(Stmt::Switch { expr, sections })
}

/// `case p when g: case q: stmts`, or `default: stmts`.
fn section(p: &mut Parser) -> PResult<Section> {
    let mut labels = Vec::new();
    loop {
        if p.eat_word(&case::WORD) {
            let pattern = p.pattern()?;
            let guard = if p.eat_word(&when::WORD) { Some(p.expr()?) } else { None };
            labels.push(Some((pattern, guard)));
        } else if p.at_word(&default::WORD) && p.nth_is(1, Punct::Colon) {
            p.bump();
            labels.push(None);
        } else {
            break;
        }
        p.need(Punct::Colon)?;
    }
    if labels.is_empty() {
        return Err(p.expected("`case` or `default`"));
    }
    let mut body = Vec::new();
    while !p.at(Punct::RBrace) && !p.at_word(&case::WORD) && !(p.at_word(&default::WORD) && p.nth_is(1, Punct::Colon)) {
        body.push(p.stmt()?);
    }
    Ok(Section { labels, body })
}
