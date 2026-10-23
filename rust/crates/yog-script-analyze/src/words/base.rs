//! `base` — `this` as its base class.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "base",
    doc: "The object as its base class: `base.Tick()` runs the replaced member; `: base(…)` runs the \
          base class's constructor first.",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<Expr> {
    let span = p.bump().span;
    Ok(Expr { kind: ExprKind::Base, span })
}
