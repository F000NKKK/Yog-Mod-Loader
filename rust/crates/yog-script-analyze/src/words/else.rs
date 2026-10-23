//! `else`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "else",
    doc: "The other branch: `if (c) … else …`, `version (…) { … } else { … }`.",
    reserved: true,
    roles: &[Role::Part],
};
