//! `pub` — a modifier.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "pub",
    doc: "Visible to other mods and files that use this one.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Pub)],
};
