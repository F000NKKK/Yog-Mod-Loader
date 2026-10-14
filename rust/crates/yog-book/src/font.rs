//! Fonts of book text.

use serde::{Deserialize, Serialize};

/// The font of a `BookPage::CustomText`: a resource-pack font id —
/// `assets/<ns>/font/<name>.json`, e.g. with a `ttf` provider — and the
/// size its 9-pixel line is drawn at.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookFont {
    /// Font id (`"example:display"`).
    pub font_id: String,
    /// Line height in book pixels (e.g. 14.0).
    pub size_px: f32,
}
