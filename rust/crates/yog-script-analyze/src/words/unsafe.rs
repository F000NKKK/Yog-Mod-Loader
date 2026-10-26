//! `unsafe`.

use crate::ast::Stmt;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "unsafe",
    doc: "A block whose values cross threads unchecked: `unsafe { Task.Run(() => cache.Hits += 1); }`. \
          For code that keeps itself safe some other way; warned of wherever it's written.",
    reserved: true,
    roles: &[Role::Stmt { parse }],
};

fn parse(p: &mut Parser) -> PResult<Stmt> {
    let at = p.bump().span;
    let body = p.block()?;
    Ok(Stmt::Unsafe(body, at))
}
