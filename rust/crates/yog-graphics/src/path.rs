//! Vector paths: contours of lines and curves, as fonts, SVG and drawn
//! shapes describe them.

use yog_data::Schema;

use crate::math::Vec2;
use crate::rect::Rect;

/// A path segment.
#[derive(Schema, Debug, Clone, Copy, PartialEq)]
#[data(tag = "s")]
pub enum Seg {
    /// Starts a contour at `x, y`.
    Move {
        x: f32,
        y: f32,
    },
    Line {
        x: f32,
        y: f32,
    },
    /// A quadratic curve through control point `x1, y1`.
    Quad {
        x1: f32,
        y1: f32,
        x: f32,
        y: f32,
    },
    /// A cubic curve through control points `x1, y1` and `x2, y2`.
    Cubic {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x: f32,
        y: f32,
    },
    /// Closes the contour.
    Close,
}

/// What `path` covers, every point (control points too) grown by `r`.
pub fn bounds(path: &[Seg], r: f32) -> Option<Rect> {
    let points = path.iter().flat_map(|s| match *s {
        Seg::Move { x, y } | Seg::Line { x, y } => vec![Vec2::new(x, y)],
        Seg::Quad { x1, y1, x, y } => vec![Vec2::new(x1, y1), Vec2::new(x, y)],
        Seg::Cubic {
            x1,
            y1,
            x2,
            y2,
            x,
            y,
        } => vec![Vec2::new(x1, y1), Vec2::new(x2, y2), Vec2::new(x, y)],
        Seg::Close => vec![],
    });
    Rect::around(points).map(|b| b.outset(r))
}

/// `path`'s contours as polylines, each point through `to` (into pixels,
/// say), curves in steps about a unit of `to`'s space long. A closed
/// contour ends on its first point.
pub fn flatten(path: &[Seg], to: impl Fn(Vec2) -> Vec2) -> Vec<Vec<Vec2>> {
    let mut out = Vec::new();
    let mut cur: Vec<Vec2> = Vec::new();
    let mut at = Vec2::ZERO;
    for seg in path {
        match *seg {
            Seg::Move { x, y } => {
                if cur.len() > 1 {
                    out.push(std::mem::take(&mut cur));
                }
                at = to(Vec2::new(x, y));
                cur = vec![at];
            }
            Seg::Line { x, y } => {
                at = to(Vec2::new(x, y));
                cur.push(at);
            }
            Seg::Quad { x1, y1, x, y } => {
                let (c, p, from) = (to(Vec2::new(x1, y1)), to(Vec2::new(x, y)), at);
                let n = steps(from.distance(c) + c.distance(p));
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    cur.push(from.lerp(c, t).lerp(c.lerp(p, t), t));
                }
                at = p;
            }
            Seg::Cubic {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let (c1, c2, p, from) = (
                    to(Vec2::new(x1, y1)),
                    to(Vec2::new(x2, y2)),
                    to(Vec2::new(x, y)),
                    at,
                );
                let n = steps(from.distance(c1) + c1.distance(c2) + c2.distance(p));
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let (a, b, c) = (from.lerp(c1, t), c1.lerp(c2, t), c2.lerp(p, t));
                    cur.push(a.lerp(b, t).lerp(b.lerp(c, t), t));
                }
                at = p;
            }
            Seg::Close => {
                if let Some(&first) = cur.first() {
                    if at != first {
                        cur.push(first);
                    }
                    at = first;
                }
                if cur.len() > 1 {
                    out.push(std::mem::take(&mut cur));
                }
                cur = vec![at];
            }
        }
    }
    if cur.len() > 1 {
        out.push(cur);
    }
    out
}

/// A curve's line every this many pixels, and the most lines one gets.
const STEP_LENGTH: f32 = 2.0;
const MAX_STEPS: f32 = 64.0;

/// Lines enough for a curve `len` long to look smooth.
fn steps(len: f32) -> usize {
    (len / STEP_LENGTH).ceil().clamp(1.0, MAX_STEPS) as usize
}
