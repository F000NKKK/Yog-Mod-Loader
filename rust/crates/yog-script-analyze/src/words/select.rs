//! `select` — a query's word.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "select",
    doc: "In a query, what each item gives: `from p in players where p.Level > 3 select p.Name`.",
    reserved: false,
    roles: &[Role::Part],
};
