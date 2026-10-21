//! Glyphs to pixels, on a [`Canvas`].

use yog_graphics::{Canvas, Color, PlacedImage, Rect, Vec2};

use crate::format::{Font, Glyph};
use crate::shape::{Shaped, glyph_bounds};

/// Glyph `glyph` of `font` drawn `scale` pixels to a unit, its text-colored
/// parts in `text_color` (white for a host that tints them), placed from
/// the glyph's origin on the baseline.
pub fn glyph(font: &Font, glyph: u32, scale: f32, text_color: Color) -> PlacedImage {
    let Some(g) = font.glyphs.get(glyph as usize) else {
        return PlacedImage::default();
    };
    let Some(b) = glyph_bounds(g) else {
        return PlacedImage::default();
    };
    let mut d = Drawing::around(b.scale(scale));
    d.glyph(g, scale, Vec2::ZERO, text_color);
    d.finish()
}

/// A shaped run drawn whole, `px_per_unit` pixels to a unit, placed from
/// the run's origin on the baseline.
pub fn run(font: &Font, shaped: &Shaped, px_per_unit: f32, text_color: Color) -> PlacedImage {
    let placed = || {
        shaped
            .glyphs
            .iter()
            .filter_map(|p| font.glyphs.get(p.glyph as usize).map(|g| (g, p)))
    };
    let Some(b) = placed()
        .filter_map(|(g, p)| {
            glyph_bounds(g).map(|r| r.scale(p.scale).translate(Vec2::new(p.x, p.y)))
        })
        .reduce(|a, b| a.union(&b))
    else {
        return PlacedImage::default();
    };
    let mut d = Drawing::around(b.scale(px_per_unit));
    for (g, p) in placed() {
        d.glyph(
            g,
            px_per_unit * p.scale,
            Vec2::new(p.x, p.y) * px_per_unit,
            text_color,
        );
    }
    d.finish()
}

/// A canvas over a box of pixels around the origin (`y` up), a pixel of
/// margin around it.
struct Drawing {
    canvas: Canvas,
    /// The canvas's top-left, in pixels from the origin, `y` up.
    left: f32,
    top: f32,
}

impl Drawing {
    fn around(b: Rect) -> Drawing {
        let (left, top) = (b.x.floor() - 1.0, b.bottom().ceil() + 1.0);
        let (w, h) = (b.right().ceil() + 1.0 - left, top - (b.y.floor() - 1.0));
        Drawing {
            canvas: Canvas::new(w as u32, h as u32),
            left,
            top,
        }
    }

    fn glyph(&mut self, g: &Glyph, s: f32, at: Vec2, text_color: Color) {
        let (left, top) = (self.left, self.top);
        self.canvas.draw(
            &g.ops,
            |p: Vec2| Vec2::new(p.x * s + at.x - left, top - (p.y * s + at.y)),
            s,
            text_color,
        );
    }

    fn finish(self) -> PlacedImage {
        PlacedImage {
            image: self.canvas.into_image(),
            x: self.left as i32,
            y: -self.top as i32,
        }
    }
}
