//! `descending` — a query's word.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "descending",
    doc: "In a query's `orderby`, highest first.",
    reserved: false,
    roles: &[Role::Part],
};
