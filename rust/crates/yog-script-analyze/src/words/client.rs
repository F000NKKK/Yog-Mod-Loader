//! `client`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "client",
    doc: "Runs on the client: `client void Welcome(…)`. Called from the server (`player.Client.F()`) it's a message.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Client)],
};
