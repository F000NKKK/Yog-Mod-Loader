//! `unchecked`, and the forms `checked` shares with it.

use crate::ast::{Expr, ExprKind, Stmt};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "unchecked",
    doc: "Whole-number arithmetic that wraps on overflow, inside a `checked` one (C#'s): `unchecked(hash * 31)`, \
          `unchecked { … }`.",
    reserved: true,
    roles: &[
        Role::Prefix { starts: Some(starts), parse: |p| expr(p, false) },
        Role::Stmt { parse: |p| stmt(p, false) },
    ],
};

fn starts(p: &Parser) -> bool {
    p.at_word(&WORD) && p.nth_is(1, Punct::LParen)
}

/// `checked(e)` / `unchecked(e)`.
pub(super) fn expr(p: &mut Parser, checked: bool) -> PResult<Expr> {
    let start = p.bump().span;
    p.need(Punct::LParen)?;
    let expr = p.expr()?;
    p.need(Punct::RParen)?;
    Ok(Expr {
        kind: ExprKind::Checked {
            expr: Box::new(expr),
            checked,
        },
        span: start.to(p.prev()),
    })
}

/// `checked { … }` / `unchecked { … }`, or a statement starting with the
/// expression form (`checked(a + b).ToString();`).
pub(super) fn stmt(p: &mut Parser, checked: bool) -> PResult<Stmt> {
    if !p.nth_is(1, Punct::LBrace) {
        let e = p.expr()?;
        p.end()?;
        return Ok(Stmt::Expr(e));
    }
    p.bump();
    Ok(Stmt::Checked(p.block()?, checked))
}
