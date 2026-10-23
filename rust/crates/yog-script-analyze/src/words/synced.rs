//! `synced`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "synced",
    doc: "Mirrored to the clients that see it: `saved synced Energy Energy = 0;`.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Synced)],
};
