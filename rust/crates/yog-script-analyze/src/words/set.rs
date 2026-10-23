//! `set`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "set",
    doc: "A property's writer: `{ get; set; }`; in its body the new value is `value`.",
    reserved: false,
    roles: &[Role::Part],
};
