//! `field`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "field",
    doc: "A property's own stored value, in its accessors: `i32 Level { get; set => field = Math.Max(value, 0); }`.",
    reserved: false,
    roles: &[Role::Part],
};
