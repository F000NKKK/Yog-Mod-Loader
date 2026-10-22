//! `first`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "first",
    doc: "The first of several matching points: `around call add first { … }`.",
    reserved: false,
    roles: &[Role::Part],
};
