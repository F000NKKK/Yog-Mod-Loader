//! `and`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "and",
    doc: "In a pattern, both hold: `level is > 3 and < 10`. Conditions join with `&&`.",
    reserved: true,
    roles: &[Role::Part],
};
