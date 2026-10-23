//! `extern`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "extern",
    doc: "A method whose body is elsewhere: `[Service(\"charge:simulate\")] static extern i64 Simulate(i64 nodes);` \
          is a call on the bus, to what the mod's Rust code (or anyone) provides there.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Extern)],
};
