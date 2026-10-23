//! `public`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "public",
    doc: "Reachable from anywhere: other files, other mods. Members are private and types internal (the mod) by default, as in C#.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Public)],
};
