//! `tune` — a modifier.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "tune",
    doc: "A value IDLE can change while the game runs: `tune let rate = 4`; a release build inlines it.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Tune)],
};
