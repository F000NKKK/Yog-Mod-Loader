//! `uses` — the locals a point asks for.

use crate::ast::Local;
use crate::parser::{PResult, Parser};
use crate::source::token::Punct;
use crate::words::{Role, Word, local};

pub static WORD: Word = Word {
    text: "uses",
    doc: "The patched method's locals a point reads (and with `var`, writes back): \
          `after tickServer(…) uses local start: Long, var local spent: Long #1 { … }`.",
    reserved: false,
    roles: &[Role::Part],
};

/// `uses local …, …`, or none.
pub(crate) fn list(p: &mut Parser) -> PResult<Vec<Local>> {
    if !p.eat_word(&WORD) {
        return Ok(Vec::new());
    }
    let mut locals = vec![local::spec(p)?];
    while p.eat(Punct::Comma) {
        locals.push(local::spec(p)?);
    }
    Ok(locals)
}
