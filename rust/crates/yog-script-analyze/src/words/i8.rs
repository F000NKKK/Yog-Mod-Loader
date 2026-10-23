//! `i8`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "i8",
    doc: "A signed 8-bit whole number (System.SByte). A literal takes it as a suffix: `5i8`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
