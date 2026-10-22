//! `call`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "call",
    doc: "A call inside the patched method, by the method called: `around call aiStep()`, `at call Vec3.add (x)`; argument types tell overloads apart: `Vec3.add(Double, Double, Double)`.",
    reserved: false,
    roles: &[Role::Part],
};
