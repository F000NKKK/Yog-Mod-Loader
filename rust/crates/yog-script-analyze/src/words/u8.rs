//! `u8`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "u8",
    doc: "An unsigned 8-bit whole number (System.Byte). A literal takes it as a suffix: `5u8`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
