//! `shared`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "shared",
    doc: "Runs on either side (the default): it may not reach a side's own classes.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Shared)],
};
