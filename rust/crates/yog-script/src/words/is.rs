//! `is` — whether a value is of a type.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::source::token::Tok;
use crate::words::{Level, Role, Word};

pub static WORD: Word = Word {
    text: "is",
    doc: "Whether a value is of a type: `e is Monster`, `!is`. With a name it's narrowed: \
          `if it is Battery b and b.charge > 0 { … }`. In a `match` arm: `is Monster m ->`.",
    reserved: true,
    roles: &[Role::Infix { level: Level::Compare, parse, negatable: true }],
};

fn parse(p: &mut Parser, left: Expr, negated: bool) -> PResult<Expr> {
    p.bump();
    let ty = p.ty_plain()?;
    let bind = if matches!(p.tok(), Tok::Ident(_)) { Some(p.name("a name")?) } else { None };
    let span = left.span.to(p.prev());
    Ok(Expr { kind: ExprKind::Is { expr: Box::new(left), ty, bind, negated }, span })
}
