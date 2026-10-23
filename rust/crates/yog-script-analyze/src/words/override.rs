//! `override`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "override",
    doc: "Replaces the base class's `virtual` or `abstract` member: `override i32 Rate => 4;`.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Override)],
};
