//! `explicit`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "explicit",
    doc: "A conversion written as a cast: `public static explicit operator i32(Energy e) => e.Amount;`, used as `(i32)energy`.",
    reserved: true,
    roles: &[Role::Part],
};
