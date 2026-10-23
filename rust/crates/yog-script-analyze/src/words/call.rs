//! `call`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "call",
    doc: "A call inside the patched method: `around call AiStep() { … }`, `at call Vec3.Add (x) { … }`.",
    reserved: false,
    roles: &[Role::Part],
};
