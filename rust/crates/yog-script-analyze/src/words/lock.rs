//! `lock`.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word, r#let, r#ref};

pub static WORD: Word = Word {
    text: "lock",
    doc: "Holds a `Lock<T>` for a statement, as C#'s: `lock (counter) { … }`; `lock (counter, ref let n) { n += 1; }` \
          lends its value, written back as the lock is let go of, however the statement ends. No `await` inside: \
          a lock isn't held across a wait.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    let at = p.bump().span;
    p.need(Punct::LParen)?;
    let target = p.braces(Parser::expr)?;
    let value = if p.eat(Punct::Comma) {
        p.need_word(&r#ref::WORD)?;
        p.need_word(&r#let::WORD)?;
        Some(p.name("the lock's value's name")?)
    } else {
        None
    };
    p.need(Punct::RParen)?;
    let body = Box::new(p.stmt()?);
    Ok(Stmt::Lock {
        target,
        value,
        body,
        at,
    })
}
