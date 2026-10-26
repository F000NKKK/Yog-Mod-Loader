//! The content kinds a Yog Script mod declares (`block Charger { … }`,
//! `item Battery { … }`): each its definition's shape (what a declaration
//! may say), a new one's defaults, and a written one read back — what the
//! checker and `yog build` make of a declaration, one source with the Rust
//! API's definitions.

use yog_data::schema::{Shape, read_value};
use yog_data::{Diagnostic, Schema, Value};

use crate::{BlockDef, ItemDef};

/// A kind of content declared in Yog Script.
pub struct ContentKind {
    /// Its word and its id's kind (`block`).
    pub kind: &'static str,
    /// Its definition's fields.
    pub shape: fn() -> Shape,
    /// A new one of `id`: every field its default.
    pub new: fn(&str) -> Value,
    /// A definition read back: what's wrong with it, if anything.
    pub check: fn(&Value) -> Result<(), Vec<Diagnostic>>,
}

/// Every kind Yog Script declares content of.
pub static KINDS: &[ContentKind] = &[
    ContentKind {
        kind: yog_ids::kind::BLOCK,
        shape: <BlockDef as Schema>::shape,
        new: |id| BlockDef::new(id).write(),
        check: |v| read_value::<BlockDef>(v).map(|_| ()),
    },
    ContentKind {
        kind: yog_ids::kind::ITEM,
        shape: <ItemDef as Schema>::shape,
        new: |id| ItemDef::new(id).write(),
        check: |v| read_value::<ItemDef>(v).map(|_| ()),
    },
];

/// The kind declared by `word`, when Yog Script declares content of it.
pub fn content_kind(word: &str) -> Option<&'static ContentKind> {
    KINDS.iter().find(|k| k.kind == word)
}
