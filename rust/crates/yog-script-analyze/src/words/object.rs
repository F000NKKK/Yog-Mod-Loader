//! `object`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "object",
    doc: "Any value (System.Object): every type converts to it; back by a cast or a pattern.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
