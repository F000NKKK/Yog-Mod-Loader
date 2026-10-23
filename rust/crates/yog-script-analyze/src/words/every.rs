//! `every`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "every",
    doc: "How often a system runs: `server system Solar(Charge charge) every 1t { … }`.",
    reserved: false,
    roles: &[Role::Part],
};
