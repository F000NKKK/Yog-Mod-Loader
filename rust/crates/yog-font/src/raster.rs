//! Glyphs to pixels, on a [`Canvas`].

use yog_graphics::{Canvas, Color, PlacedImage, Rect, Vec2, path};

use crate::format::{Font, Glyph, Op};
use crate::shape::{Shaped, glyph_bounds};

/// Glyph `glyph` of `font` drawn `scale` pixels to a unit, its text-coloured
/// parts in `text_color` (white for a host that tints them), placed from
/// the glyph's origin on the baseline.
pub fn glyph(font: &Font, glyph: u32, scale: f32, text_color: Color) -> PlacedImage {
    let Some(g) = font.glyphs.get(glyph as usize) else { return PlacedImage::default() };
    let Some(b) = glyph_bounds(g) else { return PlacedImage::default() };
    let mut d = Drawing::around(b.scale(scale));
    d.glyph(g, scale, Vec2::ZERO, text_color);
    d.finish()
}

/// A shaped run drawn whole, `px_per_unit` pixels to a unit, placed from
/// the run's origin on the baseline.
pub fn run(font: &Font, shaped: &Shaped, px_per_unit: f32, text_color: Color) -> PlacedImage {
    let placed = || {
        shaped.glyphs.iter().filter_map(|p| font.glyphs.get(p.glyph as usize).map(|g| (g, p)))
    };
    let Some(b) = placed()
        .filter_map(|(g, p)| glyph_bounds(g).map(|r| r.scale(p.scale).translate(Vec2::new(p.x, p.y))))
        .reduce(|a, b| a.union(&b))
    else {
        return PlacedImage::default();
    };
    let mut d = Drawing::around(b.scale(px_per_unit));
    for (g, p) in placed() {
        d.glyph(g, px_per_unit * p.scale, Vec2::new(p.x, p.y) * px_per_unit, text_color);
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
        Drawing { canvas: Canvas::new(w as u32, h as u32), left, top }
    }

    fn glyph(&mut self, g: &Glyph, s: f32, at: Vec2, text_color: Color) {
        let (left, top) = (self.left, self.top);
        let to_px = |p: Vec2| Vec2::new(p.x * s + at.x - left, top - (p.y * s + at.y));
        for op in &g.ops {
            match op {
                Op::Fill(f) => self.canvas.fill(&path::flatten(&f.path, to_px), f.color.unwrap_or(text_color)),
                Op::Stroke(st) => self.canvas.stroke(&path::flatten(&st.path, to_px), st.width * s, st.color.unwrap_or(text_color)),
                Op::Image(i) => {
                    let image = yog_graphics::Image {
                        width: i.px_w,
                        height: i.px_h,
                        rgba: i.pixels.iter().flat_map(|c| c.to_rgba8()).collect(),
                    };
                    let dst = Rect::from_corners(to_px(Vec2::new(i.x, i.y)), to_px(Vec2::new(i.x + i.w, i.y + i.h)));
                    self.canvas.image(&image, dst, i.tinted.then_some(text_color));
                }
            }
        }
    }

    fn finish(self) -> PlacedImage {
        PlacedImage { image: self.canvas.into_image(), x: self.left as i32, y: -self.top as i32 }
    }
}
