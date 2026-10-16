//! Text to placed glyphs: characters to glyphs, ligatures, the contextual
//! machines, then positions — advances and kerning, joined glyphs point to
//! point, joined runs fitted into the line.

use yog_graphics::{Rect, Vec2, path};

use crate::format::{Font, Glyph, Op};

/// A glyph where shaping put it, in font units from the run's origin on the
/// baseline (`y` up), drawn `scale` times its size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub glyph: u32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    /// The index of the first character (in `char`s of the text) it draws.
    pub char: usize,
}

/// A shaped run of text.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Shaped {
    pub glyphs: Vec<Placed>,
    /// How far the run moves the pen, in font units.
    pub advance: f32,
    /// Characters (indices) the font has no glyph for, not drawn.
    pub missing: Vec<usize>,
}

/// Shapes `text` in `font`.
pub fn shape(font: &Font, text: &str) -> Shaped {
    let mut missing = Vec::new();
    // (glyph, first char)
    let mut run: Vec<(u32, usize)> = Vec::new();
    for (i, c) in text.chars().enumerate() {
        match font.glyph_of(c) {
            Some(g) => run.push((g, i)),
            None => missing.push(i),
        }
    }
    let run = ligatures(font, run);
    let run = font.machines.iter().fold(run, |run, m| machine(m, run));
    let (glyphs, advance) = place(font, &run);
    Shaped { glyphs, advance, missing }
}

/// Each glyph sequence a ligature draws, longest first, replaced by it.
fn ligatures(font: &Font, run: Vec<(u32, usize)>) -> Vec<(u32, usize)> {
    if font.ligatures.is_empty() {
        return run;
    }
    let mut out = Vec::with_capacity(run.len());
    let mut i = 0;
    while i < run.len() {
        let best = font
            .ligatures
            .iter()
            .filter(|l| !l.glyphs.is_empty() && run.len() - i >= l.glyphs.len())
            .filter(|l| l.glyphs.iter().zip(&run[i..]).all(|(a, (b, _))| a == b))
            .max_by_key(|l| l.glyphs.len());
        match best {
            Some(l) => {
                out.push((l.glyph, run[i].1));
                i += l.glyphs.len();
            }
            None => {
                out.push(run[i]);
                i += 1;
            }
        }
    }
    out
}

fn machine(m: &crate::format::Machine, mut run: Vec<(u32, usize)>) -> Vec<(u32, usize)> {
    let class_of = |g: u32| m.classes.iter().find(|c| c.glyphs.contains(&g)).map(|c| c.class);
    let mut state = 0;
    for (g, _) in run.iter_mut() {
        let t = class_of(*g).and_then(|class| m.transitions.iter().find(|t| t.state == state && t.class == class));
        match t {
            Some(t) => {
                if let Some(to) = t.glyph {
                    *g = to;
                }
                state = t.next;
            }
            None => state = 0,
        }
    }
    run
}

fn place(font: &Font, run: &[(u32, usize)]) -> (Vec<Placed>, f32) {
    let glyph = |g: u32| font.glyphs.get(g as usize);
    let mut out: Vec<Placed> = Vec::with_capacity(run.len());
    let mut pen = 0.0f32;
    let mut i = 0;
    while i < run.len() {
        // A run of joined glyphs: each one's entry on the one before's exit.
        let start = out.len();
        let (mut x, mut y) = (0.0f32, 0.0f32);
        let mut prev: Option<&Glyph> = None;
        let mut j = i;
        while j < run.len() {
            let (g, c) = run[j];
            let Some(gl) = glyph(g) else {
                j += 1;
                continue;
            };
            if let Some(p) = prev {
                let (Some(exit), Some(entry)) = (p.exit, gl.entry) else { break };
                let last = out.last().copied().unwrap_or(Placed { glyph: g, x: 0.0, y: 0.0, scale: 1.0, char: c });
                x = last.x + (exit[0] - entry[0]) as f32;
                y = last.y + (exit[1] - entry[1]) as f32;
            }
            out.push(Placed { glyph: g, x, y, scale: 1.0, char: c });
            prev = Some(gl);
            j += 1;
        }
        let joined = &mut out[start..];
        if joined.len() <= 1 {
            // A glyph on its own: at the pen, kerned against the one before.
            if let Some(p) = joined.first_mut() {
                if let Some(before) = i.checked_sub(1).map(|k| run[k].0) {
                    pen += kern(font, before, p.glyph);
                }
                p.x = pen;
                pen += glyph(p.glyph).map_or(0, |g| g.advance) as f32;
            }
        } else {
            pen += fit_joined(font, joined, pen);
        }
        i = j.max(i + 1);
    }
    (out, pen)
}

/// Lays a joined run out from `pen`: as wide as its drawing plus its last
/// glyph's advance, centred and, too tall, scaled by the font's [`Fit`].
/// Returns how far it moves the pen.
///
/// [`Fit`]: crate::format::Fit
fn fit_joined(font: &Font, joined: &mut [Placed], pen: f32) -> f32 {
    let b = joined
        .iter()
        .filter_map(|p| font.glyphs.get(p.glyph as usize).and_then(glyph_bounds).map(|r| r.translate(Vec2::new(p.x, p.y))))
        .reduce(|a, b| a.union(&b))
        .unwrap_or_default();
    let (scale, dy) = match font.fit {
        Some(f) => {
            let s = if b.h > f.height && b.h > 0.0 { f.height / b.h } else { 1.0 };
            (s, f.center - b.center().y * s)
        }
        None => (1.0, 0.0),
    };
    for p in joined.iter_mut() {
        p.x = pen + (p.x - b.x) * scale;
        p.y = p.y * scale + dy;
        p.scale = scale;
    }
    let tail = joined.last().and_then(|p| font.glyphs.get(p.glyph as usize)).map_or(0, |g| g.advance);
    b.w * scale + tail as f32
}

fn kern(font: &Font, left: u32, right: u32) -> f32 {
    font.kerning.iter().find(|k| k.left == left && k.right == right).map_or(0.0, |k| k.x as f32)
}

/// What a glyph's drawing covers, in font units (`y` up; control points
/// included); `None` for a glyph that draws nothing.
pub fn glyph_bounds(g: &Glyph) -> Option<Rect> {
    g.ops
        .iter()
        .filter_map(|op| match op {
            Op::Fill(f) => path::bounds(&f.path, 0.0),
            Op::Stroke(s) => path::bounds(&s.path, s.width / 2.0),
            Op::Image(i) => Some(Rect::new(i.x, i.y, i.w, i.h)),
        })
        .reduce(|a, b| a.union(&b))
}
