//! `void`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "void",
    doc: "A method that gives nothing back: `void Greet(Player player) { … }`.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
