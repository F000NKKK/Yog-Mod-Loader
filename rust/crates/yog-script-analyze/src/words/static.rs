//! `static`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "static",
    doc: "Of the type, not of an object: `static int Count;`.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Static)],
};
