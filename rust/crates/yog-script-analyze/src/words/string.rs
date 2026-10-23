//! `string`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "string",
    doc: "Text as written: `\"charger\"`, `\"Hello, {player.Name}!\"`. Translatable text is `Text` (`t\"key\"`).",
    reserved: true,
    roles: &[Role::BuiltinType],
};
