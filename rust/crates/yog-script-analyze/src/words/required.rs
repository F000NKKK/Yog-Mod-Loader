//! `required`.

use crate::ast::Modifier;
use crate::words::{Role, Word};

pub static WORD: Word = Word {
    text: "required",
    doc: "A field or property each `new` of its type sets: `public required string Name { get; init; }` — \
          `new Item { Name = \"x\" }` (C#'s), unless the constructor called is `[SetsRequiredMembers]`.",
    reserved: true,
    roles: &[Role::Modifier(Modifier::Required)],
};
