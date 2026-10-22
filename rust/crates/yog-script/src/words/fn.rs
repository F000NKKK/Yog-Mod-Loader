//! `fn` — a function.

use crate::ast::{DeclKind, FnBody, FnDecl};
use crate::parser::{PResult, Parser};
use crate::token::Punct;
use crate::words::{Role, Word, requires};

pub static WORD: Word = Word {
    text: "fn",
    doc: "Declares a function: `fn area(w: Int, h: Int) -> Int = w * h`, or with a `{ … }` body. \
          Without a side it's `shared`; `server fn` / `client fn` run on that side, and called \
          from the other side (`x.client.f()`, `x.server.f()`) become a message.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    rest(p).map(DeclKind::Fn)
}

/// A function after its leading word (`fn`, `action`): name, generics,
/// parameters, result, guard, body (none for a bare signature).
pub(crate) fn rest(p: &mut Parser) -> PResult<FnDecl> {
    let name = p.name("a function's name")?;
    let generics = p.generics()?;
    let params = p.params()?;
    let ret = if p.eat(Punct::Arrow) { Some(p.ty()?) } else { None };
    let requires = requires::guard(p)?;
    let body = if p.eat(Punct::Eq) {
        Some(FnBody::Expr(p.expr()?))
    } else if p.at(Punct::LBrace) {
        Some(FnBody::Block(p.block()?))
    } else {
        None
    };
    Ok(FnDecl { name, generics, params, ret, requires, body })
}
