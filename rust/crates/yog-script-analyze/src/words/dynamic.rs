//! `dynamic`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "dynamic",
    doc: "A value checked only when the code runs (C#'s dynamic): members and calls on it aren't checked at build time.",
    reserved: true,
    roles: &[Role::BuiltinType],
};
