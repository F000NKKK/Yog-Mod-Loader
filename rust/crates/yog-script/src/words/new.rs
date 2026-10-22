//! `new`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "new",
    doc: "Where the patched method makes an object: `at new BlockPos { … }`.",
    reserved: false,
    roles: &[Role::Part],
};
