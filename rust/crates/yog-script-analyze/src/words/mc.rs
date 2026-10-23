//! `mc` — the game's version.

use crate::ast::{BinOp, Expr, ExprKind, VersionCond};
use crate::parser::{PResult, Parser};
use crate::source::token::{Punct, Tok};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "mc",
    doc: "The game's version, compared: `Targets = mc 1.20.1..26.2;`, `version (mc >= 1.21) { … }`.",
    reserved: false,
    roles: &[Role::Prefix { starts: Some(starts), parse }],
};

const COMPARE: &[(Punct, BinOp)] = &[
    (Punct::GtEq, BinOp::GtEq),
    (Punct::LtEq, BinOp::LtEq),
    (Punct::Gt, BinOp::Gt),
    (Punct::Lt, BinOp::Lt),
    (Punct::EqEq, BinOp::Eq),
];

fn starts(p: &Parser) -> bool {
    let version_next = matches!(p.nth(1), Tok::Version(_) | Tok::Float(_) | Tok::Int(_));
    let compare_next = COMPARE.iter().any(|(c, _)| p.nth_is(1, *c));
    p.at_word(&WORD) && (version_next || compare_next)
}

fn parse(p: &mut Parser) -> PResult<Expr> {
    let start = p.bump().span;
    let cond = cond(p)?;
    Ok(Expr { kind: ExprKind::Mc(cond), span: start.to(p.prev()) })
}

/// After `mc`: `>= 1.21` or `1.20.1..26.2`.
pub(crate) fn cond(p: &mut Parser) -> PResult<VersionCond> {
    if let Some(&(c, op)) = COMPARE.iter().find(|(c, _)| p.at(*c)) {
        p.need(c)?;
        return Ok(VersionCond::Compare { op, version: p.version()? });
    }
    let from = p.version()?;
    p.need(Punct::Range)?;
    Ok(VersionCond::Range { from, to: p.version()? })
}
