//! `internal`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "internal",
    doc: "Reachable in this mod only (the default for types).",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Internal)],
};
