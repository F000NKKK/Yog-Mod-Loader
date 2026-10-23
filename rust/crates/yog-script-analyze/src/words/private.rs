//! `private`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "private",
    doc: "Reachable only in its type (the default for members).",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Private)],
};
