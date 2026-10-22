//! `let` — a value that doesn't change.

use crate::ast::{DeclKind, Expr, Pattern, Stmt, Type};
use crate::parser::{PResult, Parser};
use crate::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "let",
    doc: "A value that doesn't change: `let name = \"charger\"`, `let (x, y) = pos`. In a declaration \
          body, `tune let rate = 4` can be changed from IDLE while the game runs.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse: decl }, Role::Stmt { parse: stmt }],
};

fn parts(p: &mut Parser) -> PResult<(Pattern, Option<Type>, Expr)> {
    p.bump();
    let pattern = p.pattern()?;
    let ty = if p.eat(Punct::Colon) { Some(p.ty()?) } else { None };
    p.need(Punct::Eq)?;
    Ok((pattern, ty, p.expr()?))
}

fn decl(p: &mut Parser) -> PResult<DeclKind> {
    parts(p).map(|(pattern, ty, value)| DeclKind::Let { pattern, ty, value })
}

fn stmt(p: &mut Parser) -> PResult<Stmt> {
    parts(p).map(|(pattern, ty, value)| Stmt::Let { pattern, ty, value })
}
