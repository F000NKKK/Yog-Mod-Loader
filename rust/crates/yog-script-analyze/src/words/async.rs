//! `async`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "async",
    doc: "A method that awaits (`await Task.Delay(5s);`): compiled into a state machine run by its side's ticks; calling it gives a `Task`.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Async)],
};
