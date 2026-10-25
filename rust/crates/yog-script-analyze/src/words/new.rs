//! `new` — an object made, or an array.

use crate::ast::{ArrayItem, Expr, ExprKind, Name, Type, TypeKind};
use crate::parser::{PResult, Parser};
use crate::source::token::{Punct, Tok};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "new",
    doc: "Makes an object: `new Pos(1, 64, 3)`, `new Battery { Count = 2 }`, `new(4)` where the type \
          is known (`Slots<Battery> Inventory = new(4);`); an array: `new i32[16]`, `new i32[4, 4]`, \
          `new string[] { \"a\", \"b\" }`, `new[] { 1, 2 }`. In a patch, where the method makes one: \
          `at new BlockPos { … }`.",
    reserved: true,
    roles: &[Role::Prefix { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<Expr> {
    let start = p.bump().span;
    // `new[] { … }`: its items' own type.
    if let Some(rank) = p.rank_ahead(0) {
        p.ranks();
        let kind = ExprKind::NewArray { elem: None, rank, sizes: Vec::new(), init: Some(items(p)?) };
        return Ok(Expr { kind, span: start.to(p.prev()) });
    }
    let ty = if p.at(Punct::LParen) { None } else { Some(p.ty()?) };
    if let Some(ty) = &ty
        && p.at(Punct::LBracket)
    {
        return sized(p, ty.clone(), start);
    }
    if let Some(Type { kind: TypeKind::Array { item, rank }, .. }) = &ty
        && p.at(Punct::LBrace)
    {
        let init = Some(items(p)?);
        let kind = ExprKind::NewArray { elem: Some((**item).clone()), rank: *rank, sizes: Vec::new(), init };
        return Ok(Expr { kind, span: start.to(p.prev()) });
    }
    let args = if p.at(Punct::LParen) { Some(p.args()?) } else { None };
    let init = if p.at(Punct::LBrace) { initializer(p)? } else { Vec::new() };
    Ok(Expr { kind: ExprKind::New { ty, args, init }, span: start.to(p.prev()) })
}

/// `new T[n, m][]… { … }?`: its sizes, then its items' own ranks.
fn sized(p: &mut Parser, item: Type, start: crate::source::token::Span) -> PResult<Expr> {
    p.need(Punct::LBracket)?;
    let sizes = p.braces(|p| p.list(Punct::RBracket, Parser::expr))?;
    let ranks = p.ranks();
    let elem = Parser::array_of(item, &ranks, start.to(p.prev()));
    let init = if p.at(Punct::LBrace) { Some(items(p)?) } else { None };
    let kind = ExprKind::NewArray { elem: Some(elem), rank: sizes.len(), sizes, init };
    Ok(Expr { kind, span: start.to(p.prev()) })
}

/// `{ a, b }`, `{ { 1, 2 }, { 3, 4 } }`: an array's items.
fn items(p: &mut Parser) -> PResult<Vec<ArrayItem>> {
    p.need(Punct::LBrace)?;
    p.braces(|p| {
        p.list(Punct::RBrace, |p| {
            if p.at(Punct::LBrace) {
                items(p).map(ArrayItem::Nested)
            } else {
                p.expr().map(ArrayItem::Value)
            }
        })
    })
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
