//! `double`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "double",
    doc: "A 64-bit number with a fraction: `0.5`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
