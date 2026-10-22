//! `of` — what slots hold.

use crate::ast::Type;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "of",
    doc: "What slots hold: `slots inventory[4] of Battery`; anything without it.",
    reserved: false,
    roles: &[Role::Part],
};

/// `of Type`, or nothing.
pub(crate) fn ty(p: &mut Parser) -> PResult<Option<Type>> {
    if !p.eat_word(&WORD) {
        return Ok(None);
    }
    p.ty_plain().map(Some)
}
