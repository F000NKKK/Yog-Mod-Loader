//! `init`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "init",
    doc: "A property set only while the object is made: `{ get; init; }`.",
    reserved: false,
    roles: &[Role::Part],
};
