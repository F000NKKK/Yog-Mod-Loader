//! `new` — an object made.

use crate::ast::{Expr, ExprKind, Name};
use crate::parser::{PResult, Parser};
use crate::source::token::{Punct, Tok};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "new",
    doc: "Makes an object: `new Pos(1, 64, 3)`, `new Battery { Count = 2 }`, `new(4)` where the type \
          is known (`Slots<Battery> Inventory = new(4);`). In a patch, where the method makes one: \
          `at new BlockPos { … }`.",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<Expr> {
    let start = p.bump().span;
    let ty = if p.at(Punct::LParen) { None } else { Some(p.ty()?) };
    let args = if p.at(Punct::LParen) { Some(p.args()?) } else { None };
    let init = if p.at(Punct::LBrace) { initializer(p)? } else { Vec::new() };
    Ok(Expr { kind: ExprKind::New { ty, args, init }, span: start.to(p.prev()) })
}

/// `{ Name = value, … }`.
fn initializer(p: &mut Parser) -> PResult<Vec<(Name, Expr)>> {
    p.need(Punct::LBrace)?;
    p.braces(|p| {
        p.list(Punct::RBrace, |p| {
            if !matches!(p.tok(), Tok::Ident(_)) {
                return Err(p.expected("a member to set"));
            }
            let name = p.name("a member")?;
            p.need(Punct::Eq)?;
            Ok((name, p.expr()?))
        })
    })
}
