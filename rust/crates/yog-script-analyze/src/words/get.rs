//! `get`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "get",
    doc: "A property's reader: `Energy Room { get; }`, `get => capacity - Energy;`.",
    reserved: false,
    roles: &[Role::Part],
};
