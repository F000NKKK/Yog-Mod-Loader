//! `sealed`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "sealed",
    doc: "A class nothing inherits, or an `override` nothing replaces further. A `switch` over a sealed hierarchy must cover each class.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Sealed)],
};
