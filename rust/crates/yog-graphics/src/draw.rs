//! Drawings: what to paint, in order — paths filled or stroked, images —
//! in a drawing's own units (`y` up). Glyphs, icons and shapes are
//! drawings; a [`Canvas`] paints them.

use yog_data::Schema;

use crate::color::Color;
use crate::image::Image;
use crate::math::Vec2;
use crate::path::{self, Seg};
use crate::raster::Canvas;
use crate::rect::Rect;

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
}

#[derive(Schema, Debug, Clone, PartialEq)]
pub struct Fill {
    pub path: Vec<Seg>,
    /// None: the current colour (what the drawing is drawn in — a text's).
    pub color: Option<Color>,
}

#[derive(Schema, Debug, Clone, PartialEq)]
pub struct Stroke {
    pub path: Vec<Seg>,
    /// How wide, in the drawing's units.
    pub width: f32,
    /// None: the current colour.
    pub color: Option<Color>,
}

/// `image` in box `rect` (`rect.x, rect.y` its bottom-left corner).
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct ImageOp {
    pub rect: Rect,
    pub image: Image,
    /// Whether the current colour tints it (an image in one colour, white
    /// to take the current one whole); else it keeps its colours.
    #[data(default)]
    pub tinted: bool,
}

/// What `ops` cover (control points included); `None` when they draw
/// nothing.
pub fn bounds(ops: &[Op]) -> Option<Rect> {
    ops.iter()
        .filter_map(|op| match op {
            Op::Fill(f) => path::bounds(&f.path, 0.0),
            Op::Stroke(s) => path::bounds(&s.path, s.width / 2.0),
            Op::Image(i) => Some(i.rect),
        })
        .reduce(|a, b| a.union(&b))
}

impl Canvas {
    /// `ops` painted: each point through `to_px` (the drawing's units to
    /// the canvas's pixels, `scale` pixels a unit), parts without a colour
    /// in `current`.
    pub fn draw(&mut self, ops: &[Op], to_px: impl Fn(Vec2) -> Vec2, scale: f32, current: Color) {
        for op in ops {
            match op {
                Op::Fill(f) => self.fill(&path::flatten(&f.path, &to_px), f.color.unwrap_or(current)),
                Op::Stroke(s) => self.stroke(&path::flatten(&s.path, &to_px), s.width * scale, s.color.unwrap_or(current)),
                Op::Image(i) => {
                    let r = i.rect;
                    let dst = Rect::from_corners(to_px(Vec2::new(r.x, r.y)), to_px(Vec2::new(r.right(), r.bottom())));
                    self.image(&i.image, dst, i.tinted.then_some(current));
                }
            }
        }
    }
}
