//! `u16`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "u16",
    doc: "An unsigned 16-bit whole number (System.UInt16). A literal takes it as a suffix: `5u16`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
