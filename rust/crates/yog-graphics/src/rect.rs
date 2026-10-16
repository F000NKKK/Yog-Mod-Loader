//! Axis-aligned rectangles.

use yog_data::Schema;

use crate::math::Vec2;

/// A rectangle: its corner `x, y` (the one with the smallest coordinates)
/// and its size.
#[derive(Schema, Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    /// The rectangle from `x0, y0` to `x1, y1` (`x1 ≥ x0`, `y1 ≥ y0`).
    pub const fn from_edges(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }

    /// The rectangle from corner `a` to corner `b`, in any order.
    pub fn from_corners(a: Vec2, b: Vec2) -> Rect {
        Rect { x: a.x.min(b.x), y: a.y.min(b.y), w: (a.x - b.x).abs(), h: (a.y - b.y).abs() }
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.right() && p.y < self.bottom()
    }

    /// The rect shrunk by `pad` (top, right, bottom, left — `y` down), never
    /// negative.
    pub fn inset(&self, pad: [f32; 4]) -> Rect {
        Rect { x: self.x + pad[3], y: self.y + pad[0], w: (self.w - pad[1] - pad[3]).max(0.0), h: (self.h - pad[0] - pad[2]).max(0.0) }
    }

    /// Grown by `r` on every side.
    pub fn outset(&self, r: f32) -> Rect {
        Rect { x: self.x - r, y: self.y - r, w: self.w + 2.0 * r, h: self.h + 2.0 * r }
    }

    pub fn translate(&self, d: Vec2) -> Rect {
        Rect { x: self.x + d.x, y: self.y + d.y, ..*self }
    }

    /// Every coordinate times `s`.
    pub fn scale(&self, s: f32) -> Rect {
        Rect::from_corners(Vec2::new(self.x * s, self.y * s), Vec2::new(self.right() * s, self.bottom() * s))
    }

    /// The overlap of two rects; `None` when they don't overlap.
    pub fn intersect(&self, o: &Rect) -> Option<Rect> {
        let (x0, y0) = (self.x.max(o.x), self.y.max(o.y));
        let (x1, y1) = (self.right().min(o.right()), self.bottom().min(o.bottom()));
        (x1 > x0 && y1 > y0).then_some(Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 })
    }

    pub fn union(&self, o: &Rect) -> Rect {
        let (x, y) = (self.x.min(o.x), self.y.min(o.y));
        Rect { x, y, w: self.right().max(o.right()) - x, h: self.bottom().max(o.bottom()) - y }
    }

    /// The smallest rectangle around `points`; `None` for none.
    pub fn around(points: impl IntoIterator<Item = Vec2>) -> Option<Rect> {
        let mut it = points.into_iter();
        let first = it.next()?;
        let (lo, hi) = it.fold((first, first), |(lo, hi), p| (Vec2::new(lo.x.min(p.x), lo.y.min(p.y)), Vec2::new(hi.x.max(p.x), hi.y.max(p.y))));
        Some(Rect::from_corners(lo, hi))
    }
}
