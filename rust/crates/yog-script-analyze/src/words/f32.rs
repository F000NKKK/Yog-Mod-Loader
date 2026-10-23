//! `f32`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "f32",
    doc: "A 32-bit floating-point number (System.Single). A literal takes it as a suffix: `5f32`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
