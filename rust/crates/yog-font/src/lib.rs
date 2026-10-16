//! yog-font — the one kind of font in Yog: what `.yui` screens, books,
//! textures, the IDE and the game's own text (chat, signs, tooltips) all
//! draw text with.
//!
//! A font ([`Font`], a `.yfont` file) is drawing, as a PDF is: its glyphs
//! are paths filled or stroked and pictures, never a notion of what they're
//! for. Whatever it's built from — a TrueType file, a picture of glyphs,
//! pixels drawn in the editor, a script's rules — it ends up this.
//!
//! Text becomes glyphs by [`shape`]: characters to glyphs, ligatures,
//! contextual forms by state machines (a glyph replaced by what comes
//! before it), glyphs joined point to point (cursive scripts, and runes
//! drawn stroke by stroke), joined runs fitted into the line. [`raster`]
//! draws glyphs or a whole run to pixels.

pub mod format;
pub mod raster;
pub mod shape;

pub use format::{CharGlyph, Fit, Font, Glyph, GlyphClass, Kern, Ligature, Machine, Op, Seg, Transition};
pub use raster::Bitmap;
pub use shape::{Placed, Shaped, shape};

#[cfg(test)]
mod tests;
