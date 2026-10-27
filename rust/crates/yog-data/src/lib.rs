//! Yog Data: Substrate Data (one model, text, binary, frames and schemas)
//! with Yog's domain declared over it — the kinds of content, tags, mod
//! resources, translation keys, texts players read, images and colours.
//! See `docs/design/yog-data.md`.
//!
//! Everything of Substrate Data is here as it is (`yog_data::Value`,
//! `yog_data::Schema`, `yog_data::to_bytes`); Yog's own types are the
//! modules below.

pub use substrate_data::*;

mod built_in;
mod color;
mod content;
mod expected;
pub mod found;
pub mod export;
mod image;
mod res;
mod tag;
mod shown;

pub use built_in::built_in;
pub use color::Color;
pub use expected::expected;
pub use content::{BlockId, EntityId, ItemId, RecipeId, kinds};
pub use image::Image;
pub use res::Res;
pub use tag::{AnyRefOrTag, ItemIdOrTag, OneOrTag, TagId};
pub use shown::{Lang, Text};
