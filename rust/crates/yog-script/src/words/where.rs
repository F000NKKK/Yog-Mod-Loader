//! `where`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "where",
    doc: "The matching point where a condition holds: `at call add where arg x is strength (x) { … }`.",
    reserved: false,
    roles: &[Role::Part],
};
