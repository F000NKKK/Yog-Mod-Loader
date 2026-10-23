//! `i64`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "i64",
    doc: "A signed 64-bit whole number (System.Int64). A literal takes it as a suffix: `5i64`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
