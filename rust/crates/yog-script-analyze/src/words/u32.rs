//! `u32`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "u32",
    doc: "An unsigned 32-bit whole number (System.UInt32). A literal takes it as a suffix: `5u32`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
