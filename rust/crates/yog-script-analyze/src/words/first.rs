//! `first`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "first",
    doc: "The first of several matching points: `at call Vec3.Add first (x) { … }`.",
    reserved: false,
    roles: &[Role::Part],
};
