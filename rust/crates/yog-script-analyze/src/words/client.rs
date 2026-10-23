//! `client` — a modifier.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "client",
    doc: "Runs on a client: `client fn showCharge(level: Int) { … }`. The server reaches it with `player.client.showCharge(5)` — a message made for it.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Client)],
};
