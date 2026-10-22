//! `requires` — who may call a function.

use crate::ast::Expr;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "requires",
    doc: "A guard before a function's body: `server fn fill(src: Sender) requires src.isOp { … }`. \
          A call that fails it is refused (a command's caller is told why).",
    reserved: false,
    roles: &[Role::Part],
};

/// `requires cond`, or nothing.
pub(crate) fn guard(p: &mut Parser) -> PResult<Option<Expr>> {
    if !p.eat_word(&WORD) {
        return Ok(None);
    }
    p.expr_no_brace().map(Some)
}
