//! `ascending` — a query's word.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "ascending",
    doc: "In a query's `orderby`, lowest first (as without it).",
    reserved: false,
    roles: &[Role::Part],
};
