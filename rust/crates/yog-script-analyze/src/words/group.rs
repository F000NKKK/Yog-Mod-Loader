//! `group` — a query's word.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "group",
    doc: "In a query, items grouped by a key: `from p in players group p by p.Level`.",
    reserved: false,
    roles: &[Role::Part],
};
