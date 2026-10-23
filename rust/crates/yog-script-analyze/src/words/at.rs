//! `at` — a point inside the method: an argument, a field, a `new`.

use crate::ast::{Point, Selector};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word, call, local, new, read, write};

pub static WORD: Word = Word {
    text: "at",
    doc: "A point inside the patched method: an argument of a call by its name (`at call Vec3.Add (x) { x * 0.5 }`), \
          a field's read or write (`at read Speed { … }`), where an object is made (`at new BlockPos { … }`).",
    reserved: false,
    roles: &[Role::Point { parse }],
};

fn parse(p: &mut Parser) -> PResult<Point> {
    p.bump();
    if p.eat_word(&call::WORD) {
        return argument(p);
    }
    if p.eat_word(&new::WORD) {
        let ty = p.ty()?;
        let (select, uses) = after_target(p)?;
        return Ok(Point::New { ty, select, uses, body: p.block()? });
    }
    let write = if p.eat_word(&write::WORD) {
        true
    } else {
        p.need_word(&read::WORD)?;
        false
    };
    let field = p.any_name("a field")?;
    let (select, uses) = after_target(p)?;
    Ok(Point::Field { write, field, select, uses, body: p.block()? })
}

/// `call Callee selector? (arg) { … }`.
fn argument(p: &mut Parser) -> PResult<Point> {
    let callee = p.callee()?;
    let select = p.selector()?;
    p.need(Punct::LParen)?;
    let arg = p.name("the argument's name")?;
    p.need(Punct::RParen)?;
    let body = p.value_block()?;
    Ok(Point::Arg { callee, select, arg, body })
}

fn after_target(p: &mut Parser) -> PResult<(Option<Selector>, Vec<crate::ast::Local>)> {
    let select = p.selector()?;
    Ok((select, local::uses(p)?))
}
