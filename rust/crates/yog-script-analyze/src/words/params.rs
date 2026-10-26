//! `params`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "params",
    doc: "A method's last parameter taking the call's arguments past the others as an array, as C#'s: `void Log(string format, params object[] args)` called `Log(\"{0} {1}\", a, b)`.",
    reserved: true,
    roles: &[Role::Part],
};
