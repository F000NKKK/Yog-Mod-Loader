//! A canvas to draw on in pixels (`y` down): shapes filled with signed-area
//! coverage (anti-aliased, non-zero winding), strokes as the union of
//! their segments with round joins and caps, images sampled into place.

use crate::geometry::math::Vec2;
use crate::geometry::rect::Rect;
use crate::pixels::color::Color;
use crate::pixels::image::Image;
use crate::pixels::mask::Mask;

/// The thinnest stroke's half-width: thinner ones vanish between pixels.
const MIN_STROKE_RADIUS: f32 = 0.35;
/// How far inside the right edge a line is kept, so it lands in the last column.
const EDGE_INSET: f32 = 0.001;
/// The middle of a line's second cell, in cells from its first cell's left edge.
const SECOND_CELL_MIDDLE: f32 = 1.5;

/// Pixels being drawn, premultiplied, in floats.
#[derive(Debug, Clone)]
pub struct Canvas {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

impl Canvas {
    /// A transparent `w × h` canvas.
    pub fn new(w: u32, h: u32) -> Canvas {
        Canvas {
            w: w as usize,
            h: h as usize,
            px: vec![[0.0; 4]; (w * h) as usize],
        }
    }

    pub fn width(&self) -> u32 {
        self.w as u32
    }

    pub fn height(&self) -> u32 {
        self.h as u32
    }

    /// The inside of `contours` (closed polylines; an open one is closed)
    /// painted `color`.
    pub fn fill(&mut self, contours: &[Vec<Vec2>], color: Color) {
        let lines = contours.iter().flat_map(|c| {
            c.iter()
                .zip(c.iter().cycle().skip(1))
                .map(|(a, b)| (*a, *b))
        });
        let cov = coverage(self.w, self.h, lines);
        self.paint(&cov, color);
    }

    /// `polylines` drawn `width` wide in `color`, round-joined and capped.
    pub fn stroke(&mut self, polylines: &[Vec<Vec2>], width: f32, color: Color) {
        let r = (width / 2.0).max(MIN_STROKE_RADIUS);
        let mut pieces: Vec<Vec<Vec2>> = Vec::new();
        for line in polylines {
            for w in line.windows(2) {
                pieces.extend(segment_quad(w[0], w[1], r));
            }
            for &p in line {
                pieces.push(circle(p, r));
            }
        }
        let lines = pieces.iter().flat_map(|c| {
            c.iter()
                .zip(c.iter().cycle().skip(1))
                .map(|(a, b)| (*a, *b))
        });
        let cov = coverage(self.w, self.h, lines);
        self.paint(&cov, color);
    }

    /// `image` sampled (nearest) into `dst`, its colors multiplied by
    /// `tint` if given.
    pub fn image(&mut self, image: &Image, dst: Rect, tint: Option<Color>) {
        if image.is_empty() {
            return;
        }
        let tint = tint.map(Color::to_f32);
        self.sampled(image.width, image.height, dst, |u, v| {
            let c = image.pixel(u, v).to_f32();
            Some(match tint {
                Some(t) => [c[0] * t[0], c[1] * t[1], c[2] * t[2], c[3] * t[3]],
                None => c,
            })
        });
    }

    /// `mask` in box `dst` (pixels), its set pixels `color`.
    pub fn mask(&mut self, mask: &Mask, dst: Rect, color: Color) {
        let c = color.to_f32();
        self.sampled(mask.width, mask.height, dst, |u, v| {
            mask.get(u, v).then_some(c)
        });
    }

    /// A `width × height` picture in box `dst`: each canvas pixel in it
    /// takes the picture's pixel under its centre, `sample` giving its color
    /// (none: nothing there).
    fn sampled(
        &mut self,
        width: u32,
        height: u32,
        dst: Rect,
        sample: impl Fn(u32, u32) -> Option<[f32; 4]>,
    ) {
        if width == 0 || height == 0 || dst.w <= 0.0 || dst.h <= 0.0 {
            return;
        }
        for py in
            (dst.y.floor().max(0.0) as usize)..(dst.bottom().ceil().min(self.h as f32) as usize)
        {
            for px in
                (dst.x.floor().max(0.0) as usize)..(dst.right().ceil().min(self.w as f32) as usize)
            {
                let u = ((px as f32 + 0.5 - dst.x) / dst.w * width as f32).floor();
                let v = ((py as f32 + 0.5 - dst.y) / dst.h * height as f32).floor();
                if u < 0.0 || v < 0.0 || u >= width as f32 || v >= height as f32 {
                    continue;
                }
                if let Some(c) = sample(u as u32, v as u32) {
                    over(&mut self.px[py * self.w + px], c);
                }
            }
        }
    }

    /// `color` over the canvas where `cov` (0..1 a pixel) covers it.
    fn paint(&mut self, cov: &[f32], color: Color) {
        let c = color.to_f32();
        for (px, &k) in self.px.iter_mut().zip(cov) {
            if k > 0.0 {
                over(px, [c[0], c[1], c[2], c[3] * k]);
            }
        }
    }

    /// The drawing, straight alpha.
    pub fn into_image(self) -> Image {
        let mut rgba = Vec::with_capacity(self.w * self.h * 4);
        for p in &self.px {
            let a = p[3];
            let un = |c: f32| if a > 0.0 { c / a } else { 0.0 };
            rgba.extend(Color::from_f32([un(p[0]), un(p[1]), un(p[2]), a]).to_rgba8());
        }
        Image {
            width: self.w as u32,
            height: self.h as u32,
            rgba,
        }
    }
}

/// Straight `src` over premultiplied `dst`.
fn over(dst: &mut [f32; 4], src: [f32; 4]) {
    let a = src[3];
    for i in 0..3 {
        dst[i] = src[i] * a + dst[i] * (1.0 - a);
    }
    dst[3] = a + dst[3] * (1.0 - a);
}

/// A rectangle around segment `a → b`, `r` to each side, wound one way.
fn segment_quad(a: Vec2, b: Vec2, r: f32) -> Option<Vec<Vec2>> {
    let n = (b - a).normalize().perp() * r;
    (a != b).then(|| oriented(vec![a + n, b + n, b - n, a - n]))
}

/// A circle as a polygon, wound one way.
fn circle(c: Vec2, r: f32) -> Vec<Vec2> {
    let n = ((r * 2.0).ceil() as usize * 4).clamp(8, 48);
    oriented(
        (0..n)
            .map(|i| {
                let t = i as f32 / n as f32 * std::f32::consts::TAU;
                c + Vec2::new(t.cos(), t.sin()) * r
            })
            .collect(),
    )
}

/// `poly` wound the same way as every other stroke piece, so where they
/// overlap they add up instead of cancelling.
fn oriented(mut poly: Vec<Vec2>) -> Vec<Vec2> {
    let area: f32 = poly
        .iter()
        .zip(poly.iter().cycle().skip(1))
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum();
    if area < 0.0 {
        poly.reverse();
    }
    poly
}

/// Coverage of the shape `lines` bound (closed contours, pixels), each
/// pixel 0..1: signed area accumulated along each row.
fn coverage(w: usize, h: usize, lines: impl Iterator<Item = (Vec2, Vec2)>) -> Vec<f32> {
    let mut acc = vec![0.0f32; w * h + 4];
    if w == 0 || h == 0 {
        return Vec::new();
    }
    for (p0, p1) in lines {
        accumulate(&mut acc, w, h, p0, p1);
    }
    let mut sum = 0.0;
    acc.truncate(w * h);
    for a in acc.iter_mut() {
        sum += *a;
        *a = sum.abs().min(1.0);
    }
    acc
}

/// Adds line `p0 → p1`'s signed area to the rows it crosses.
fn accumulate(acc: &mut [f32], w: usize, h: usize, p0: Vec2, p1: Vec2) {
    if (p0.y - p1.y).abs() <= f32::EPSILON {
        return;
    }
    let (dir, p0, p1) = if p0.y < p1.y {
        (1.0, p0, p1)
    } else {
        (-1.0, p1, p0)
    };
    let clamp_x = |x: f32| x.clamp(0.0, w as f32 - EDGE_INSET);
    let dxdy = (p1.x - p0.x) / (p1.y - p0.y);
    let mut x = p0.x;
    if p0.y < 0.0 {
        x -= p0.y * dxdy;
    }
    let y_start = p0.y.max(0.0) as usize;
    let y_end = (p1.y.ceil().max(0.0) as usize).min(h);
    for y in y_start..y_end {
        let row = y * w;
        let dy = ((y + 1) as f32).min(p1.y) - (y as f32).max(p0.y);
        let xnext = x + dxdy * dy;
        let d = dy * dir;
        let (x0, x1) = if x < xnext {
            (clamp_x(x), clamp_x(xnext))
        } else {
            (clamp_x(xnext), clamp_x(x))
        };
        let x0f = x0.floor();
        let x0i = x0f as usize;
        let x1i = x1.ceil() as usize;
        if x1i <= x0i + 1 {
            let xm = 0.5 * (x0 + x1) - x0f;
            acc[row + x0i] += d - d * xm;
            acc[row + x0i + 1] += d * xm;
        } else {
            let s = 1.0 / (x1 - x0);
            let x0r = x0 - x0f;
            let a0 = 0.5 * s * (1.0 - x0r) * (1.0 - x0r);
            let x1r = x1 - x1.ceil() + 1.0;
            let am = 0.5 * s * x1r * x1r;
            acc[row + x0i] += d * a0;
            if x1i == x0i + 2 {
                acc[row + x0i + 1] += d * (1.0 - a0 - am);
            } else {
                let a1 = s * (SECOND_CELL_MIDDLE - x0r);
                acc[row + x0i + 1] += d * (a1 - a0);
                for xi in x0i + 2..x1i - 1 {
                    acc[row + xi] += d * s;
                }
                let a2 = a1 + (x1i - x0i - 3) as f32 * s;
                acc[row + x1i - 1] += d * (1.0 - a2 - am);
            }
            acc[row + x1i] += d * am;
        }
        x = xnext;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_fill_and_strokes_round() {
        let mut c = Canvas::new(20, 20);
        c.fill(
            &[vec![
                Vec2::new(2.0, 2.0),
                Vec2::new(10.0, 2.0),
                Vec2::new(10.0, 10.0),
                Vec2::new(2.0, 10.0),
            ]],
            Color::WHITE,
        );
        c.stroke(
            &[vec![Vec2::new(4.0, 15.0), Vec2::new(16.0, 15.0)]],
            2.0,
            Color::rgb(0xff0000),
        );
        let img = c.into_image();
        assert_eq!(img.pixel(5, 5), Color::WHITE);
        assert_eq!(img.pixel(11, 5).a(), 0);
        assert_eq!(img.pixel(10, 15), Color::rgb(0xff0000));
        assert_eq!(img.pixel(10, 17).a(), 0);
    }

    #[test]
    fn a_square_with_a_hole_keeps_it() {
        let mut c = Canvas::new(12, 12);
        let outer = vec![
            Vec2::new(1.0, 1.0),
            Vec2::new(11.0, 1.0),
            Vec2::new(11.0, 11.0),
            Vec2::new(1.0, 11.0),
        ];
        let inner = vec![
            Vec2::new(4.0, 4.0),
            Vec2::new(4.0, 8.0),
            Vec2::new(8.0, 8.0),
            Vec2::new(8.0, 4.0),
        ];
        c.fill(&[outer, inner], Color::WHITE);
        let img = c.into_image();
        assert_eq!(img.pixel(2, 2), Color::WHITE);
        assert_eq!(img.pixel(6, 6).a(), 0);
    }
}
