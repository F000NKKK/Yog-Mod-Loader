//! `uses`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "uses",
    doc: "The patched method's locals a patch reads (`var` ones it writes back): `uses local i64 start, var local i64 spent #1`.",
    reserved: false,
    roles: &[Role::Part],
};
