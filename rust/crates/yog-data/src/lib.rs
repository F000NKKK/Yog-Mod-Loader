//! Yog Data: one data model, text syntax, binary form and schema system for
//! every Yog format, the build, the ABI and the IDE — the way Minecraft has
//! NBT. See `docs/design/yog-data.md`.
//!
//! - [`Value`]: the model — JSON's values plus typed ones (content
//!   references, tags, mod resources, Yog text, images, colors);
//! - [`text`]: JSON with comments, trailing commas and unquoted keys,
//!   parsed with the position of every value;
//! - [`typed`]: the one grammar of the typed values' string forms
//!   (`@item:minecraft:stick`, `@tag:item:minecraft:planks`, `@Strings.title`);
//! - [`binary`]: the compact tagged form content crosses the ABI in.

// `::yog_data::…` paths of `#[derive(Schema)]` work inside this crate too.
extern crate self as yog_data;

pub mod atom;
pub mod binary;
mod borrowed;
pub mod diagnostic;
pub mod export;
pub mod frame;
pub mod io;
#[cfg(feature = "json")]
pub mod json;
pub mod patch;
mod stream;
pub mod text;
#[cfg(feature = "toml")]
pub mod toml;
pub mod typed;
mod value;
pub mod with;

pub use atom::Atom;
pub use borrowed::{MapRef, ValueRef};
pub use bumpalo::Bump;
pub use diagnostic::{Diagnostic, Errors, Severity};
pub use stream::{Stream, StreamWriter};
pub use text::{Doc, LineCol, Span, Spans, SyntaxError};
pub use value::{Map, Path, Ref, Res, Seg, Value};
pub mod schema;
pub mod source;

pub use schema::{
    AnyRefOrTag, BlockId, Color, EntityId, Id, Image, ItemId, ItemIdOrTag, Kind, Kinds, OneOrList,
    OneOrTag, Ordered, Read, Reader, RecipeId, Resolve, Schema, Shape, TagId, Text, built_in,
    from_doc_with, from_text, from_value, from_value_with, kinds, placed, read_bytes,
    read_ref_value, read_value, to_bytes, to_text,
};
pub use yog_data_derive::Schema;
