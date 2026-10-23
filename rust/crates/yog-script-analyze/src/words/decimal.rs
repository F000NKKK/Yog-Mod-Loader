//! `decimal`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "decimal",
    doc: "A 128-bit decimal number, exact for money-like values (System.Decimal).",
    reserved: true,
    roles: &[Role::BuiltinType],
};
