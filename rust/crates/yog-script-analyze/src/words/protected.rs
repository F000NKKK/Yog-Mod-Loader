//! `protected`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "protected",
    doc: "Reachable from the type and the types inheriting it: `protected i32 Charge;`.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Protected)],
};
