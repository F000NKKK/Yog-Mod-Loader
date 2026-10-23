//! `as` — a value as another type.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Level, Role, Word};

pub static WORD: Word = Word {
    text: "as",
    doc: "A value as another type: `self as Chargeable`; `as?` gives `none` when it isn't one. \
          Lossy conversions are calls (`x.floor()`), never `as`. Also renames in `use a.b as c`.",
    reserved: true,
    roles: &[Role::Infix { level: Level::Convert, parse, negatable: false }],
};

fn parse(p: &mut Parser, left: Expr, _negated: bool) -> PResult<Expr> {
    p.bump();
    let checked = p.eat(Punct::Question);
    let ty = p.ty_plain()?;
    let span = left.span.to(p.prev());
    Ok(Expr { kind: ExprKind::As { expr: Box::new(left), ty, checked }, span })
}
