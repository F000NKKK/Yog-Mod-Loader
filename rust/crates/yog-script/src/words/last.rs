//! `last`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "last",
    doc: "The last of several matching points.",
    reserved: false,
    roles: &[Role::Part],
};
