//! `checked`.

use crate::parser::Parser;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "checked",
    doc: "Whole-number arithmetic that panics on overflow instead of wrapping (C#'s): `checked(a * b)`, \
          `checked { total += part; }`; a narrowing cast out of range panics too.",
    reserved: true,
    roles: &[
        Role::Prefix { starts: Some(starts), parse: |p| super::unchecked::expr(p, true) },
        Role::Stmt { parse: |p| super::unchecked::stmt(p, true) },
    ],
};

fn starts(p: &Parser) -> bool {
    p.at_word(&WORD) && p.nth_is(1, crate::source::token::Punct::LParen)
}
