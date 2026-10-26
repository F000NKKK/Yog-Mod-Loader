//! `orderby` — a query's word.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "orderby",
    doc: "In a query, the items' order by keys: `orderby p.Level descending, p.Name`.",
    reserved: false,
    roles: &[Role::Part],
};
