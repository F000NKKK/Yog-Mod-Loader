//! Glyphs to pixels: paths filled with signed-area coverage (anti-aliased,
//! non-zero winding), strokes as the union of their segments, round joins
//! and caps, pictures sampled into place.

use crate::format::{Font, Glyph, Op, Seg};
use crate::shape::{Bounds, Shaped, glyph_bounds};

/// Pixels, RGBA row by row from the top (straight alpha), and where they
/// sit: their top-left corner `left` pixels right of and `top` pixels above
/// the origin they were drawn for.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
    pub rgba: Vec<u8>,
}

impl Bitmap {
    /// Pixel `x, y`'s `[r, g, b, a]`.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }
}

/// Glyph `glyph` of `font` drawn `scale` pixels to a unit, its text-coloured
/// parts in `text_color` (`0xAARRGGBB`; white for a host that tints them).
pub fn glyph(font: &Font, glyph: u32, scale: f32, text_color: u32) -> Bitmap {
    let Some(g) = font.glyphs.get(glyph as usize) else { return Bitmap::default() };
    let b = glyph_bounds(g).scaled(scale);
    let mut canvas = Canvas::around(b);
    canvas.draw(g, scale, 0.0, 0.0, text_color);
    canvas.finish()
}

/// A shaped run drawn whole, `px_per_unit` pixels to a unit.
pub fn run(font: &Font, shaped: &Shaped, px_per_unit: f32, text_color: u32) -> Bitmap {
    let mut b = Bounds::EMPTY;
    for p in &shaped.glyphs {
        if let Some(g) = font.glyphs.get(p.glyph as usize) {
            b = b.union(glyph_bounds(g).scaled(p.scale).offset(p.x, p.y));
        }
    }
    let mut canvas = Canvas::around(b.scaled(px_per_unit));
    for p in &shaped.glyphs {
        if let Some(g) = font.glyphs.get(p.glyph as usize) {
            canvas.draw(g, px_per_unit * p.scale, p.x * px_per_unit, p.y * px_per_unit, text_color);
        }
    }
    canvas.finish()
}

/// Premultiplied RGBA in floats, and the box it covers.
struct Canvas {
    w: usize,
    h: usize,
    /// Pixel coordinates of the canvas's top-left, `y` up.
    left: f32,
    top: f32,
    px: Vec<[f32; 4]>,
}

impl Canvas {
    /// A canvas covering `b` (pixels, `y` up), a pixel of margin around it.
    fn around(b: Bounds) -> Canvas {
        if b.is_empty() {
            return Canvas { w: 0, h: 0, left: 0.0, top: 0.0, px: Vec::new() };
        }
        let (left, top) = (b.x0.floor() - 1.0, b.y1.ceil() + 1.0);
        let (w, h) = ((b.x1.ceil() + 1.0 - left) as usize, (top - (b.y0.floor() - 1.0)) as usize);
        Canvas { w, h, left, top, px: vec![[0.0; 4]; w * h] }
    }

    /// Font units `x, y` (scaled by `s`, moved by `dx, dy` pixels) to the
    /// canvas's pixels, `y` down.
    fn to_px(&self, s: f32, dx: f32, dy: f32, x: f32, y: f32) -> (f32, f32) {
        (x * s + dx - self.left, self.top - (y * s + dy))
    }

    fn draw(&mut self, g: &Glyph, s: f32, dx: f32, dy: f32, text_color: u32) {
        if self.w == 0 {
            return;
        }
        for op in &g.ops {
            match op {
                Op::Fill { path, color } => {
                    let lines = self.flatten(path, s, dx, dy, true);
                    let cov = coverage(self.w, self.h, lines.iter().flatten().copied());
                    self.paint(&cov, color.unwrap_or(text_color));
                }
                Op::Stroke { path, width, color } => {
                    let r = (width * s / 2.0).max(0.35);
                    let mut polys: Vec<Vec<(f32, f32)>> = Vec::new();
                    for line in self.flatten(path, s, dx, dy, false) {
                        let pts: Vec<(f32, f32)> = line.iter().map(|l| l.0).chain(line.last().map(|l| l.1)).collect();
                        for w in pts.windows(2) {
                            polys.push(segment_quad(w[0], w[1], r));
                        }
                        for &p in &pts {
                            polys.push(circle(p, r));
                        }
                    }
                    let lines = polys.iter().flat_map(|p| p.iter().zip(p.iter().cycle().skip(1)).map(|(a, b)| (*a, *b)));
                    let cov = coverage(self.w, self.h, lines);
                    self.paint(&cov, color.unwrap_or(text_color));
                }
                Op::Image { x, y, w, h, px_w, px_h, argb, tinted } => self.image(s, dx, dy, [*x, *y, *w, *h], (*px_w, *px_h), argb, tinted.then_some(text_color)),
            }
        }
    }

    /// A path's contours as lines in pixels; `close` closes open ones (fills).
    fn flatten(&self, path: &[Seg], s: f32, dx: f32, dy: f32, close: bool) -> Vec<Vec<((f32, f32), (f32, f32))>> {
        let mut out = Vec::new();
        let mut cur: Vec<((f32, f32), (f32, f32))> = Vec::new();
        let (mut start, mut at) = ((0.0, 0.0), (0.0, 0.0));
        let mut end = |cur: &mut Vec<((f32, f32), (f32, f32))>, at: (f32, f32), start: (f32, f32), close: bool| {
            if close && at != start {
                cur.push((at, start));
            }
            if !cur.is_empty() {
                out.push(std::mem::take(cur));
            }
        };
        for seg in path {
            match *seg {
                Seg::Move { x, y } => {
                    end(&mut cur, at, start, close);
                    at = self.to_px(s, dx, dy, x, y);
                    start = at;
                }
                Seg::Line { x, y } => {
                    let p = self.to_px(s, dx, dy, x, y);
                    cur.push((at, p));
                    at = p;
                }
                Seg::Quad { cx, cy, x, y } => {
                    let (c, p) = (self.to_px(s, dx, dy, cx, cy), self.to_px(s, dx, dy, x, y));
                    let (n, from) = (steps(at, c, p), at);
                    for i in 1..=n {
                        let t = i as f32 / n as f32;
                        let q = lerp(lerp(from, c, t), lerp(c, p, t), t);
                        cur.push((at, q));
                        at = q;
                    }
                    at = p;
                }
                Seg::Cubic { c1x, c1y, c2x, c2y, x, y } => {
                    let (c1, c2, p) = (self.to_px(s, dx, dy, c1x, c1y), self.to_px(s, dx, dy, c2x, c2y), self.to_px(s, dx, dy, x, y));
                    let n = steps(at, c1, p).max(steps(c1, c2, p));
                    let from = at;
                    for i in 1..=n {
                        let t = i as f32 / n as f32;
                        let (a, b, c) = (lerp(from, c1, t), lerp(c1, c2, t), lerp(c2, p, t));
                        let q = lerp(lerp(a, b, t), lerp(b, c, t), t);
                        cur.push((at, q));
                        at = q;
                    }
                    at = p;
                }
                Seg::Close => {
                    if at != start {
                        cur.push((at, start));
                    }
                    at = start;
                    end(&mut cur, at, start, false);
                }
            }
        }
        end(&mut cur, at, start, close);
        out
    }

    /// `color` (`0xAARRGGBB`) over the canvas where `cov` covers it.
    fn paint(&mut self, cov: &[f32], color: u32) {
        let c = unpack(color);
        for (px, &k) in self.px.iter_mut().zip(cov) {
            if k > 0.0 {
                over(px, [c[0], c[1], c[2], c[3] * k]);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn image(&mut self, s: f32, dx: f32, dy: f32, [x, y, w, h]: [f32; 4], (pw, ph): (u32, u32), argb: &[u32], tint: Option<u32>) {
        if pw == 0 || ph == 0 || argb.len() < (pw * ph) as usize {
            return;
        }
        let (x0, y0) = self.to_px(s, dx, dy, x, y + h);
        let (x1, y1) = self.to_px(s, dx, dy, x + w, y);
        let tint = tint.map(unpack);
        for py in (y0.floor().max(0.0) as usize)..(y1.ceil().min(self.h as f32) as usize) {
            for px in (x0.floor().max(0.0) as usize)..(x1.ceil().min(self.w as f32) as usize) {
                // Nearest pixel of the picture at this pixel's centre.
                let u = ((px as f32 + 0.5 - x0) / (x1 - x0) * pw as f32).floor();
                let v = ((py as f32 + 0.5 - y0) / (y1 - y0) * ph as f32).floor();
                if u < 0.0 || v < 0.0 || u >= pw as f32 || v >= ph as f32 {
                    continue;
                }
                let mut c = unpack(argb[v as usize * pw as usize + u as usize]);
                if let Some(t) = tint {
                    c = [c[0] * t[0], c[1] * t[1], c[2] * t[2], c[3] * t[3]];
                }
                over(&mut self.px[py * self.w + px], c);
            }
        }
    }

    fn finish(self) -> Bitmap {
        let mut rgba = Vec::with_capacity(self.w * self.h * 4);
        for p in &self.px {
            let a = p[3];
            let un = |c: f32| if a > 0.0 { (c / a).clamp(0.0, 1.0) } else { 0.0 };
            rgba.extend([un(p[0]), un(p[1]), un(p[2]), a.clamp(0.0, 1.0)].map(|v| (v * 255.0 + 0.5) as u8));
        }
        Bitmap { width: self.w as u32, height: self.h as u32, left: self.left as i32, top: self.top as i32, rgba }
    }
}

fn lerp(a: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// Lines enough for a curve through `a, c, b` to look smooth.
fn steps(a: (f32, f32), c: (f32, f32), b: (f32, f32)) -> usize {
    let d = |p: (f32, f32), q: (f32, f32)| ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
    ((d(a, c) + d(c, b)) / 2.0).ceil().clamp(1.0, 64.0) as usize
}

/// `0xAARRGGBB` as premultiplied-ready `[r, g, b, a]` in 0..1 (straight).
fn unpack(c: u32) -> [f32; 4] {
    [(c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff, c >> 24].map(|v| v as f32 / 255.0)
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
fn segment_quad(a: (f32, f32), b: (f32, f32), r: f32) -> Vec<(f32, f32)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        return Vec::new();
    }
    let (nx, ny) = (-dy / len * r, dx / len * r);
    oriented(vec![(a.0 + nx, a.1 + ny), (b.0 + nx, b.1 + ny), (b.0 - nx, b.1 - ny), (a.0 - nx, a.1 - ny)])
}

/// A circle as a polygon, wound one way.
fn circle(c: (f32, f32), r: f32) -> Vec<(f32, f32)> {
    let n = ((r * 2.0).ceil() as usize * 4).clamp(8, 48);
    oriented((0..n).map(|i| {
        let t = i as f32 / n as f32 * std::f32::consts::TAU;
        (c.0 + r * t.cos(), c.1 + r * t.sin())
    }).collect())
}

/// `poly` wound the same way as every other stroke piece, so overlaps add
/// up instead of cancelling.
fn oriented(mut poly: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    let area: f32 = poly.iter().zip(poly.iter().cycle().skip(1)).map(|(a, b)| a.0 * b.1 - b.0 * a.1).sum();
    if area < 0.0 {
        poly.reverse();
    }
    poly
}

/// Coverage of the shape `lines` bound (closed contours, pixels), each
/// pixel 0..1: signed area accumulated along each row (non-zero winding,
/// anti-aliased).
fn coverage(w: usize, h: usize, lines: impl Iterator<Item = ((f32, f32), (f32, f32))>) -> Vec<f32> {
    let mut acc = vec![0.0f32; w * h + 4];
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
fn accumulate(acc: &mut [f32], w: usize, h: usize, p0: (f32, f32), p1: (f32, f32)) {
    if (p0.1 - p1.1).abs() <= f32::EPSILON {
        return;
    }
    let (dir, p0, p1) = if p0.1 < p1.1 { (1.0, p0, p1) } else { (-1.0, p1, p0) };
    let clamp_x = |x: f32| x.clamp(0.0, w as f32 - 0.001);
    let dxdy = (p1.0 - p0.0) / (p1.1 - p0.1);
    let mut x = p0.0;
    if p0.1 < 0.0 {
        x -= p0.1 * dxdy;
    }
    let y_start = p0.1.max(0.0) as usize;
    let y_end = (p1.1.ceil().max(0.0) as usize).min(h);
    for y in y_start..y_end {
        let row = y * w;
        let dy = ((y + 1) as f32).min(p1.1) - (y as f32).max(p0.1);
        let xnext = x + dxdy * dy;
        let d = dy * dir;
        let (x0, x1) = if x < xnext { (clamp_x(x), clamp_x(xnext)) } else { (clamp_x(xnext), clamp_x(x)) };
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
                let a1 = s * (1.5 - x0r);
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
