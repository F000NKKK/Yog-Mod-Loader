//! `var` — a local whose type is its value's.

use crate::ast::{Binding, Stmt};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "var",
    doc: "A local whose type is its value's, assignable again (as `let mut`): `var moved = Math.Min(Energy, into.Room);`; \
          `var (a, b) = pair;` takes a tuple apart. In a pattern, `var e` binds anything.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    let s = local(p)?;
    p.end()?;
    Ok(s)
}

/// `var name (= value)?` or `var (a, b) = value`, without the `;`.
pub(crate) fn local(p: &mut Parser) -> PResult<Stmt> {
    p.need_word(&WORD)?;
    let binding = binding(p)?;
    let value = if p.eat(Punct::Eq) { Some(p.expr()?) } else { None };
    Ok(Stmt::Local { ty: None, binding, value, mutable: true })
}

/// `name` or `(a, b)`.
pub(crate) fn binding(p: &mut Parser) -> PResult<Binding> {
    if p.eat(Punct::LParen) {
        return Ok(Binding::Tuple(p.list(Punct::RParen, |p| p.name("a name"))?));
    }
    Ok(Binding::One(p.name("a name")?))
}
