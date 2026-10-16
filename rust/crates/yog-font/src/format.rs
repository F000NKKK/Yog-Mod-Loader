//! The `.yfont` format: what a font is once built, whatever it was made
//! from (TrueType, a picture of glyphs, pixels drawn in an editor, a
//! script's rules). Yog Data, binary.
//!
//! Units are the font's own (`units_per_em` to an em), `y` up from the
//! baseline. A glyph is drawing — paths filled or stroked, pictures —
//! never a notion of what it's for.

use yog_data::Schema;

/// A font: its metrics, its glyphs, which characters they draw and the
/// rules that turn text into glyphs ([`crate::shape`]).
#[derive(Schema, Debug, Clone, PartialEq, Default)]
pub struct Font {
    /// How many units an em is: a glyph drawn at size `s` is scaled by
    /// `s / units_per_em`.
    pub units_per_em: u32,
    /// How far above the baseline its tallest glyphs reach.
    pub ascent: i32,
    /// How far below the baseline its glyphs reach (negative: below).
    pub descent: i32,
    /// Space between one line's descent and the next one's ascent.
    #[data(default)]
    pub line_gap: i32,
    /// Its glyphs; a glyph is named by its index.
    pub glyphs: Vec<Glyph>,
    /// Which glyph draws each character.
    pub chars: Vec<CharGlyph>,
    /// Glyph sequences one glyph draws (`f` `i` → `fi`), the longest first.
    #[data(default)]
    pub ligatures: Vec<Ligature>,
    /// Contextual forms: glyphs replaced by what comes before them.
    #[data(default)]
    pub machines: Vec<Machine>,
    /// Extra space between pairs of glyphs.
    #[data(default)]
    pub kerning: Vec<Kern>,
    /// How a run of glyphs joined to each other ([`Glyph::entry`]) that
    /// grows too tall is fitted into the line.
    pub fit: Option<Fit>,
}

/// One glyph: how far it moves the pen, and its drawing.
#[derive(Schema, Debug, Clone, PartialEq, Default)]
pub struct Glyph {
    /// What it's called where the font is made (`a`, `heart`, `rune_ne`).
    #[data(default)]
    pub name: String,
    /// How far the pen moves after it.
    #[data(default)]
    pub advance: i32,
    /// Its drawing, in order.
    #[data(default)]
    pub ops: Vec<Op>,
    /// Where the glyph before it, if that one has an [`Glyph::exit`], joins it
    /// — joined glyphs are placed point to point, up and down too (cursive
    /// scripts; runes drawn stroke by stroke).
    pub entry: Option<[i32; 2]>,
    /// Where the glyph after it joins it.
    pub exit: Option<[i32; 2]>,
}

/// One step of a glyph's drawing.
#[derive(Schema, Debug, Clone, PartialEq)]
#[data(tag = "op")]
pub enum Op {
    /// A path's inside painted (non-zero winding).
    Fill {
        path: Vec<Seg>,
        /// `0xAARRGGBB`; none: the text's colour.
        color: Option<u32>,
    },
    /// A path's outline painted, `width` units wide, round-joined and capped.
    Stroke {
        path: Vec<Seg>,
        width: f32,
        color: Option<u32>,
    },
    /// A picture in the box `x, y` (bottom-left) `w × h` units.
    Image {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        /// Its pixels' size.
        px_w: u32,
        px_h: u32,
        /// `0xAARRGGBB`, row by row from the top.
        argb: Vec<u32>,
        /// Whether the text's colour tints it (a picture in one colour,
        /// white for the text's own); else it keeps its colours.
        #[data(default)]
        tinted: bool,
    },
}

/// A path segment.
#[derive(Schema, Debug, Clone, Copy, PartialEq)]
#[data(tag = "s")]
pub enum Seg {
    /// Starts a contour at `x, y`.
    Move { x: f32, y: f32 },
    Line { x: f32, y: f32 },
    /// A quadratic curve through control point `cx, cy`.
    Quad { cx: f32, cy: f32, x: f32, y: f32 },
    /// A cubic curve through control points `c1`, `c2`.
    Cubic { c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32 },
    /// Closes the contour.
    Close,
}

/// Character `char` (a Unicode scalar) is drawn by glyph `glyph`.
#[derive(Schema, Debug, Clone, Copy, PartialEq)]
pub struct CharGlyph {
    pub char: u32,
    pub glyph: u32,
}

/// Glyphs `glyphs`, in a row, are drawn by glyph `glyph`.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct Ligature {
    pub glyphs: Vec<u32>,
    pub glyph: u32,
}

/// A state machine over the glyphs of a run, left to right (as Apple's
/// `morx` tables): each glyph's class and the machine's state pick a
/// transition, which may put another glyph in its place and moves to its
/// next state. A glyph with no transition from the state leaves it as it is
/// and starts the machine over.
#[derive(Schema, Debug, Clone, PartialEq, Default)]
pub struct Machine {
    /// Glyphs of each class; a glyph in none has no transitions.
    pub classes: Vec<GlyphClass>,
    pub transitions: Vec<Transition>,
}

#[derive(Schema, Debug, Clone, PartialEq)]
pub struct GlyphClass {
    pub class: u32,
    pub glyphs: Vec<u32>,
}

/// From state `state` (0: the start), a glyph of class `class` moves the
/// machine to `next`, put glyph `glyph` in its place if there is one.
#[derive(Schema, Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    pub state: u32,
    pub class: u32,
    pub next: u32,
    pub glyph: Option<u32>,
}

/// `left` followed by `right` are `x` units further apart.
#[derive(Schema, Debug, Clone, Copy, PartialEq)]
pub struct Kern {
    pub left: u32,
    pub right: u32,
    pub x: i32,
}

/// A run of joined glyphs taller than `height` units is scaled down to it,
/// centred on `center` units above the baseline.
#[derive(Schema, Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    pub height: f32,
    pub center: f32,
}

impl Font {
    /// A built font from its file's bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Font, String> {
        yog_data::read_bytes::<Font>(bytes).map_err(|e| format!("{e:?}"))
    }

    /// The font's file.
    pub fn to_bytes(&self) -> Vec<u8> {
        yog_data::binary::encode(&self.write())
    }

    /// The glyph that draws `c`.
    pub fn glyph_of(&self, c: char) -> Option<u32> {
        let c = c as u32;
        self.chars.iter().find(|m| m.char == c).map(|m| m.glyph)
    }
}
