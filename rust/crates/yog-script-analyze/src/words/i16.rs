//! `i16`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "i16",
    doc: "A signed 16-bit whole number (System.Int16). A literal takes it as a suffix: `5i16`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
