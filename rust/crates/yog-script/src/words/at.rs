//! `at` — a place inside a patched method.

use crate::ast::{Callee, Name, Point, TypeKind};
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word, call, new, read, uses, write};

pub static WORD: Word = Word {
    text: "at",
    doc: "A place inside the patched method: `at call Vec3.add (x) { x * 0.5 }` changes an argument by its \
          name; `at read field { … }` / `at write field { … }` run at a field's access; `at new Type { … }` \
          where one is made.",
    reserved: false,
    roles: &[Role::Point { parse }],
};

fn parse(p: &mut Parser) -> PResult<Point> {
    p.bump();
    if p.eat_word(&call::WORD) {
        return argument(p);
    }
    if p.eat_word(&new::WORD) {
        let ty = p.ty_plain()?;
        let select = p.selector()?;
        let uses = uses::list(p)?;
        return Ok(Point::New { ty, select, uses, body: p.block()? });
    }
    let write = if p.eat_word(&write::WORD) {
        true
    } else {
        p.need_word(&read::WORD)?;
        false
    };
    let field = p.name("a field")?;
    let select = p.selector()?;
    let uses = uses::list(p)?;
    Ok(Point::Field { write, field, select, uses, body: p.block()? })
}

/// `at call X (arg) { … }`. Written `X(arg)`, the one name in the
/// brackets is the argument, not a type list.
fn argument(p: &mut Parser) -> PResult<Point> {
    let mut callee = p.callee()?;
    let select = p.selector()?;
    let arg = if p.eat(Punct::LParen) {
        let arg = p.name("an argument's name")?;
        p.need(Punct::RParen)?;
        arg
    } else {
        lone_name(&mut callee).ok_or_else(|| p.expected("`(argument)`"))?
    };
    Ok(Point::Arg { callee, select, arg, body: p.block()? })
}

/// `X(name)` read as the callee `X` and its argument `name`.
fn lone_name(callee: &mut Callee) -> Option<Name> {
    let [one] = callee.args.as_deref()? else { return None };
    let TypeKind::Path { path, args } = &one.kind else { return None };
    let [name] = path.as_slice() else { return None };
    if !args.is_empty() {
        return None;
    }
    let name = name.clone();
    callee.args = None;
    Some(name)
}
