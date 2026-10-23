//! `virtual`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "virtual",
    doc: "A member an inheriting class may replace: `virtual i32 Rate => 1;`.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Virtual)],
};
