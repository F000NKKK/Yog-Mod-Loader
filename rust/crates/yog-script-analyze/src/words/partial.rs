//! `partial`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "partial",
    doc: "A method whose body is generated: `[LoggerMessage(LogLevel.Information, \"Moved {Count} items\")] \
          static partial void Moved(ILogger logger, i32 count);` logs as fast as a hand-written check of its level.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Partial)],
};
