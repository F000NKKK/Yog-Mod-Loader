//! `float`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "float",
    doc: "A 32-bit number with a fraction: `2.0f`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
