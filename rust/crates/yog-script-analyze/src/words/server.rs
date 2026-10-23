//! `server` — a modifier.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "server",
    doc: "Runs on the server: `server fn`, `server on tick`. Code on one side can't reach the other's classes — a client class in server code is a build error; a call across sides (`x.client.f()`) is a message.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Server)],
};
