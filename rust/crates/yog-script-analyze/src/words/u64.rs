//! `u64`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "u64",
    doc: "An unsigned 64-bit whole number (System.UInt64). A literal takes it as a suffix: `5u64`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
