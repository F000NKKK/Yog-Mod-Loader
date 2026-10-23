//! `let` — a local, unchangeable unless `mut`.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word, r#mut, var};

pub static WORD: Word = Word {
    text: "let",
    doc: "A local, never assigned again: `let room = into.Room;`; `let mut left = 20t;` may be. \
          `let x: i32 = …` names its type, `let (a, b) = pair;` takes a tuple apart.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    p.bump();
    let mutable = p.eat_word(&r#mut::WORD);
    let binding = var::binding(p)?;
    let ty = if p.eat(Punct::Colon) { Some(p.ty()?) } else { None };
    let value = if p.eat(Punct::Eq) { Some(p.expr()?) } else { None };
    p.end()?;
    Ok(Stmt::Local { ty, binding, value, mutable })
}
