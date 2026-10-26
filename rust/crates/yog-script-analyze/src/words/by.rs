//! `by` — a query's word.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "by",
    doc: "In a query's `group … by key`, the key.",
    reserved: false,
    roles: &[Role::Part],
};
