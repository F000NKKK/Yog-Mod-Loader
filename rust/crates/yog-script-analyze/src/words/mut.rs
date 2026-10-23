//! `mut` — a local assignable again.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "mut",
    doc: "Makes a `let` assignable again and lendable by `ref`: `let mut left = 20t;`.",
    reserved: true,
    roles: &[Role::Part],
};
