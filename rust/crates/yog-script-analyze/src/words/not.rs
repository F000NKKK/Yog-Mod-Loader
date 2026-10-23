//! `not`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "not",
    doc: "In a pattern, it doesn't hold: `x is not null`. A condition is negated with `!`.",
    reserved: true,
    roles: &[Role::Part],
};
