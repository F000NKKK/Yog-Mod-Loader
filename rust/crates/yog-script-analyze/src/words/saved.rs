//! `saved`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "saved",
    doc: "Kept in the world's save: `saved i32 Visits = 0;`. The codec is made for it; renames are told with `was`.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Saved)],
};
