//! `const`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "const",
    doc: "A value known at build time: `const i32 MaxTier = 3;`.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Const)],
};
