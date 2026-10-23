//! `when`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "when",
    doc: "A switch arm's guard: `var e when e > 3 => …`.",
    reserved: true,
    roles: &[Role::Part],
};
