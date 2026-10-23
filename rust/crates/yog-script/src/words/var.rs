//! `var` — a value that changes.

use crate::ast::{DeclKind, Stmt, VarDecl};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word, was};

pub static WORD: Word = Word {
    text: "var",
    doc: "A value that changes: `var count = 0`. On content and in state, `saved` keeps it in the world's \
          save, `synced` shows it to the clients that see its owner, and `was old: Type` reads an old \
          save's field: `saved synced var energy: Energy in 0..1000 = 0 was power: Int`.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse: decl }, Role::Stmt { parse: stmt }],
};

fn rest(p: &mut Parser) -> PResult<VarDecl> {
    p.bump();
    let name = p.name("a variable")?;
    let ty = if p.eat(Punct::Colon) { Some(p.ty()?) } else { None };
    let value = if p.eat(Punct::Eq) { Some(p.expr()?) } else { None };
    let was = was::migration(p)?;
    Ok(VarDecl { name, ty, value, was })
}

fn decl(p: &mut Parser) -> PResult<DeclKind> {
    rest(p).map(DeclKind::Var)
}

fn stmt(p: &mut Parser) -> PResult<Stmt> {
    rest(p).map(|v| Stmt::Var { name: v.name, ty: v.ty, value: v.value })
}
