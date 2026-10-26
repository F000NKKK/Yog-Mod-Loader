//! `equals` — a query's word.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "equals",
    doc: "In a query's `join … on a equals b`, the key the other side is matched by.",
    reserved: false,
    roles: &[Role::Part],
};
