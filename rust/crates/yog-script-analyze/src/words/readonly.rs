//! `readonly`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "readonly",
    doc: "Set once, in its declaration or a constructor: `readonly Pos Origin;`.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Readonly)],
};
