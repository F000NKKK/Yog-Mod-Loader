//! `tune`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "tune",
    doc: "A value IDLE can change in a running game: `tune i32 Rate = 4;` (inlined in a release build).",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Tune)],
};
