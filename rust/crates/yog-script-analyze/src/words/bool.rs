//! `bool`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "bool",
    doc: "`true` or `false`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
