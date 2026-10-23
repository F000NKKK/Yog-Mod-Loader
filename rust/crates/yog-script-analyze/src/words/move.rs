//! `move` — ownership handed over.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "move",
    doc: "Hands a value over: `void Keep(move Stream s)`, called `Keep(move stream)`; the caller's \
          `stream` can't be used after. Who owns a value disposes it (`IDisposable`).",
    reserved: true,
    roles: &[Role::Part],
};
