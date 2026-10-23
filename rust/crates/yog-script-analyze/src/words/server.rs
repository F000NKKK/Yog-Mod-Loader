//! `server`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "server",
    doc: "Runs on the server: `server void Fill(…)`, `server on Tick { … }`, `server system …`. Called from a client (`x.Server.F()`) it's a message.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Server)],
};
