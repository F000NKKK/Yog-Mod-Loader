//! `char`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "char",
    doc: "One character: `'R'`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
