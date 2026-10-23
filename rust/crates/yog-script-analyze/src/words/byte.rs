//! `byte`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "byte",
    doc: "An 8-bit whole number.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
