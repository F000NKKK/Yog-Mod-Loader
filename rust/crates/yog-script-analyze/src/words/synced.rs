//! `synced` — a modifier.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "synced",
    doc: "Shown to the clients that see its owner (a mirror): `synced var energy = 0`. A client reads it without a message per read.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Synced)],
};
