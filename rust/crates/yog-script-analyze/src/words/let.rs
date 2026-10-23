//! `let` — a local, unchangeable unless `mut`.

use crate::ast::{Binding, Stmt};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word, r#mut};

pub static WORD: Word = Word {
    text: "let",
    doc: "A local, never assigned again: `let room = into.Room;`; `let mut left = 20t;` may be. \
          `let x: i32 = …` names its type, `let (a, b) = pair;` takes a tuple apart. In a pattern \
          (`Mode.Charging(let rate)`) and a `foreach` (`foreach (let b in Inventory)`) it binds a name.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    let s = local(p)?;
    p.end()?;
    Ok(s)
}

/// `let mut? binding (: T)? (= value)?`, without the `;`.
pub(crate) fn local(p: &mut Parser) -> PResult<Stmt> {
    p.need_word(&WORD)?;
    let mutable = p.eat_word(&r#mut::WORD);
    let binding = binding(p)?;
    let ty = if p.eat(Punct::Colon) { Some(p.ty()?) } else { None };
    let value = if p.eat(Punct::Eq) { Some(p.expr()?) } else { None };
    Ok(Stmt::Local { ty, binding, value, mutable })
}

/// `name` or `(a, b)`.
pub(crate) fn binding(p: &mut Parser) -> PResult<Binding> {
    if p.eat(Punct::LParen) {
        return Ok(Binding::Tuple(p.list(Punct::RParen, |p| p.name("a name"))?));
    }
    Ok(Binding::One(p.name("a name")?))
}
