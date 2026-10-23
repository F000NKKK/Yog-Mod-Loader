//! `ref` — a value lent to change.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "ref",
    doc: "A value lent to change (Rust's `&mut`): `void Charge(ref i32 energy)`, called \
          `Charge(ref energy)` on a `let mut`. It lives only for the call: never kept in a field \
          or a lambda.",
    reserved: true,
    roles: &[Role::Part],
};
