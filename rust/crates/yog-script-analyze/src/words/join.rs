//! `join` — a query's word.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "join",
    doc: "In a query, items of another sequence matched by keys: `join t in teams on p.Team equals t.Name`.",
    reserved: false,
    roles: &[Role::Part],
};
