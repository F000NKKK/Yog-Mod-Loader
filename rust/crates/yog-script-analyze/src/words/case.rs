//! `case` — a `switch` statement's label.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "case",
    doc: "A `switch` statement's label: `case 1:`, `case Tier.Iron or Tier.Gold:`, `case Monster m when m.Health < 5:` \
          (C#'s). Several labels may share a section; a section ends with `break`, `return` or `continue`.",
    reserved: true,
    roles: &[Role::Part],
};
