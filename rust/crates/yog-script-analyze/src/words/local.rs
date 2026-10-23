//! `local` — a local of the patched method.

use crate::ast::{Local, Point};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word, uses, var};

pub static WORD: Word = Word {
    text: "local",
    doc: "A local of the patched method, by name (the class index keeps the game's local names) or by \
          type and order (`#1`): `uses local i64 start` reads it, `var local` writes it back; inside a \
          method, `local f32 speed = speed * 2;` changes it (`@ModifyVariable`).",
    reserved: false,
    roles: &[Role::Point { parse }],
};

fn parse(p: &mut Parser) -> PResult<Point> {
    let local = declared(p, false)?;
    p.need(Punct::Eq)?;
    let value = p.expr()?;
    p.end()?;
    Ok(Point::Local { local, value })
}

/// `local Type name (#n)?` (after `var` when `write`).
fn declared(p: &mut Parser, write: bool) -> PResult<Local> {
    p.need_word(&WORD)?;
    let ty = p.ty()?;
    let name = p.name("a local's name")?;
    let ordinal = if p.eat(Punct::Hash) { Some(p.int("the local's place among its type")?) } else { None };
    Ok(Local { write, ty, name, ordinal })
}

/// `uses local …, var local …`, or none.
pub(crate) fn uses(p: &mut Parser) -> PResult<Vec<Local>> {
    if !p.eat_word(&uses::WORD) {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    loop {
        let write = p.eat_word(&var::WORD);
        out.push(declared(p, write)?);
        if !p.eat(Punct::Comma) {
            return Ok(out);
        }
    }
}
