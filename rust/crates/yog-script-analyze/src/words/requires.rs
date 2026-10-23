//! `requires`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "requires",
    doc: "A guard of a command or a server method called by clients: `server void Fill(Sender source) requires source.IsOp { … }`.",
    reserved: false,
    roles: &[Role::Part],
};
