//! `on` — a query's word.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "on",
    doc: "In a query's `join`, what's matched: `on outerKey equals innerKey`.",
    reserved: false,
    roles: &[Role::Part],
};
