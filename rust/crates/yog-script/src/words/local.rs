//! `local` — a local variable of the patched method.

use crate::ast::{Local, Point};
use crate::parser::{PResult, Parser};
use crate::token::Punct;
use crate::words::{Role, Word, r#var};

pub static WORD: Word = Word {
    text: "local",
    doc: "A local variable of the patched method, by name where the game keeps names, else by type \
          and order (`#1`: the second of its type). In `uses` it's read (`var local`: written back); \
          inside a method, `local x: Float = x * 2` changes it where it's set.",
    reserved: false,
    roles: &[Role::Point { parse }],
};

/// `var? local name: Type (#n)?`.
pub(crate) fn spec(p: &mut Parser) -> PResult<Local> {
    let write = p.eat_word(&r#var::WORD);
    p.need_word(&WORD)?;
    let name = p.name("a local")?;
    p.need(Punct::Colon)?;
    let ty = p.ty_plain()?;
    let ordinal = if p.eat(Punct::Hash) { Some(p.int("an ordinal")?) } else { None };
    Ok(Local { write, name, ty, ordinal })
}

fn parse(p: &mut Parser) -> PResult<Point> {
    let local = spec(p)?;
    p.need(Punct::Eq)?;
    Ok(Point::Local { local, value: p.expr()? })
}
