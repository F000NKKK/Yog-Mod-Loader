//! `macro` — a declarative macro.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "macro",
    doc: "A declarative macro, as Rust's `macro_rules!`: rules matched against a call's tokens, \
          `macro Square { ($x:expr) => { $x * $x }; }`, called `Square!(3)`. Expanded before \
          anything else reads the code.",
    reserved: true,
    roles: &[Role::Part],
};
