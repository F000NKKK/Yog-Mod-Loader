//! `int`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "int",
    doc: "A 32-bit whole number.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
