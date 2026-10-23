//! `f64`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "f64",
    doc: "A 64-bit floating-point number (System.Double); a fractional literal's type. A literal takes it as a suffix: `5f64`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
