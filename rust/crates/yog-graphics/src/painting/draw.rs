//! Drawings: what to paint, in order — paths filled or stroked, images —
//! in a drawing's own units (`y` up). Glyphs, icons and shapes are
//! drawings; a [`Canvas`] paints them.

use yog_data::Schema;

use crate::geometry::math::Vec2;
use crate::geometry::path::{self, Seg};
use crate::geometry::rect::Rect;
use crate::painting::raster::Canvas;
use crate::pixels::color::Color;
use crate::pixels::image::Image;
use crate::pixels::mask::Mask;

/// One step of a drawing.
#[derive(Schema, Debug, Clone, PartialEq)]
#[data(tag = "op")]
pub enum Op {
    /// A path's inside painted (non-zero winding).
    Fill(Fill),
    /// A path's outline painted, round-joined and capped.
    Stroke(Stroke),
    /// An image in a box.
    Image(ImageOp),
    /// A one-color picture, a bit a pixel, in a box (a pixel font's glyph).
    Mask(MaskOp),
}

#[derive(Schema, Debug, Clone, PartialEq)]
pub struct Fill {
    pub path: Vec<Seg>,
    /// None: the current color (what the drawing is drawn in — a text's).
    pub color: Option<Color>,
}

#[derive(Schema, Debug, Clone, PartialEq)]
pub struct Stroke {
    pub path: Vec<Seg>,
    /// How wide, in the drawing's units.
    pub width: f32,
    /// None: the current color.
    pub color: Option<Color>,
}

/// `image` in box `rect` (`rect.x, rect.y` its bottom-left corner).
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct ImageOp {
    pub rect: Rect,
    pub image: Image,
    /// Whether the current color tints it (an image in one color, white
    /// to take the current one whole); else it keeps its colors.
    #[data(default)]
    pub tinted: bool,
}

/// `mask` in box `rect` (`rect.x, rect.y` its bottom-left corner), its set
/// pixels `color` (none: the current color — a text's).
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct MaskOp {
    pub rect: Rect,
    pub mask: Mask,
    pub color: Option<Color>,
}

/// What `ops` cover (control points included); `None` when they draw
/// nothing.
pub fn bounds(ops: &[Op]) -> Option<Rect> {
    ops.iter()
        .filter_map(|op| match op {
            Op::Fill(f) => path::bounds(&f.path, 0.0),
            Op::Stroke(s) => path::bounds(&s.path, s.width / 2.0),
            Op::Image(i) => Some(i.rect),
            Op::Mask(m) => Some(m.rect),
        })
        .reduce(|a, b| a.union(&b))
}

impl Canvas {
    /// `ops` painted: each point through `to_px` (the drawing's units to
    /// the canvas's pixels, `scale` pixels a unit), parts without a color
    /// in `current`.
    pub fn draw(&mut self, ops: &[Op], to_px: impl Fn(Vec2) -> Vec2, scale: f32, current: Color) {
        for op in ops {
            match op {
                Op::Fill(f) => {
                    self.fill(&path::flatten(&f.path, &to_px), f.color.unwrap_or(current))
                }
                Op::Stroke(s) => self.stroke(
                    &path::flatten(&s.path, &to_px),
                    s.width * scale,
                    s.color.unwrap_or(current),
                ),
                Op::Image(i) => {
                    self.image(
                        &i.image,
                        px_box(i.rect, &to_px),
                        i.tinted.then_some(current),
                    );
                }
                Op::Mask(m) => {
                    self.mask(&m.mask, px_box(m.rect, &to_px), m.color.unwrap_or(current))
                }
            }
        }
    }
}

/// Box `r` of a drawing in the canvas's pixels.
fn px_box(r: Rect, to_px: &impl Fn(Vec2) -> Vec2) -> Rect {
    Rect::from_corners(
        to_px(Vec2::new(r.x, r.y)),
        to_px(Vec2::new(r.right(), r.bottom())),
    )
}
