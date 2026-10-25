//! `implicit`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "implicit",
    doc: "A conversion that happens by itself where its target is wanted: `public static implicit operator Energy(i32 amount) => new Energy(amount);`.",
    reserved: true,
    roles: &[Role::Part],
};
