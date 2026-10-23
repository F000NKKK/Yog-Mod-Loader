//! `i32`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "i32",
    doc: "A signed 32-bit whole number (System.Int32); a whole literal's type. A literal takes it as a suffix: `5i32`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
