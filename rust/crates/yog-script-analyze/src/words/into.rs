//! `into` — a query's word.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "into",
    doc: "In a query, a `group`'s or a `select`'s result named, the query going on from it; a `join`'s matches named.",
    reserved: false,
    roles: &[Role::Part],
};
