//! `long`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "long",
    doc: "A 64-bit whole number.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
