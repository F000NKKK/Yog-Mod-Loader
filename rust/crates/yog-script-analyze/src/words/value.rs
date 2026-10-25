//! `value`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "value",
    doc: "A `set`'s or an `init`'s new value: `set { field = Math.Clamp(value, 0, 100); }`, `set { cells[i] = value; }`.",
    reserved: false,
    roles: &[Role::Part],
};
