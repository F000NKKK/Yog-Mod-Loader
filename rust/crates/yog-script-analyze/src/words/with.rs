//! `with` — a copy of a record or a struct, some members changed.

use crate::ast::{Expr, ExprKind};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Level, Role, Word};

pub static WORD: Word = Word {
    text: "with",
    doc: "A copy of a record or a struct with some members changed, as C#'s: `pos with { Y = 64 }` \
          (the original unchanged).",
    reserved: true,
    roles: &[Role::Infix { level: Level::Switch, parse }],
};

fn parse(p: &mut Parser, left: Expr) -> PResult<Expr> {
    p.bump();
    p.need(Punct::LBrace)?;
    let inits = p.braces(|p| {
        p.list(Punct::RBrace, |p| {
            let name = p.name("a member")?;
            p.need(Punct::Eq)?;
            Ok((name, p.expr()?))
        })
    })?;
    let span = left.span.to(p.prev());
    Ok(Expr { kind: ExprKind::With { expr: Box::new(left), inits }, span })
}
