//! `or`.

use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "or",
    doc: "In a pattern, either holds: `Tier.Wood or Tier.Stone => 1`. Conditions join with `||`.",
    reserved: true,
    roles: &[Role::Part],
};
