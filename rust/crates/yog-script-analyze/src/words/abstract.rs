//! `abstract`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "abstract",
    doc: "A class only inherited, never made (`abstract class Machine`), or a member it leaves to them: `abstract void Tick();`.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Abstract)],
};
