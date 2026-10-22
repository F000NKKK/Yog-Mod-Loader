//! `shared` — a modifier.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "shared",
    doc: "Runs on either side and touches neither's own classes: the default for a function without a side.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Shared)],
};
