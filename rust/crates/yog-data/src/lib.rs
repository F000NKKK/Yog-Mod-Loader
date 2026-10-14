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

pub mod binary;
pub mod text;
pub mod typed;
mod value;

pub use text::{Doc, LineCol, Span, Spans, SyntaxError};
pub use value::{Map, Path, Ref, Res, Seg, Value};
pub mod schema;

pub use schema::{
    BlockId, Color, EntityId, Errors, Id, Image, ItemId, ItemIdOrTag, Kind, OneOrTag, Ordered,
    Read, Reader, RecipeId, Schema, Shape, TagId, Text, from_text, from_value, kinds, to_text,
};
pub use yog_data_derive::Schema;
