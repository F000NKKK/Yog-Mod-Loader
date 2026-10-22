//! A Unifont-sized font (every glyph a 16 × 16 mask, as `yog import font
//! yog:font/uniform` writes them): loading it, finding glyphs (by binary
//! search, and the linear scan it replaced), shaping and drawing a line.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use yog_font::{CharGlyph, Font, Glyph};
use yog_graphics::{Color, Mask, MaskOp, Op, Rect};

/// Unifont's glyph count, about.
const GLYPHS: u32 = 57_000;
/// Its cell, pixels.
const CELL: u32 = 16;
/// Font units a pixel is.
const UNIT: f32 = 8.0;
/// Where its glyphs start (CJK and the rest above the basic planes' start).
const FIRST: u32 = 0x20;
/// A line of text, characters.
const LINE: usize = 200;

fn unifont() -> Font {
    let bytes = Mask::row_bytes(CELL) * CELL as usize;
    let mut font = Font {
        units_per_em: (CELL as f32 * UNIT) as u32,
        ascent: 14 * UNIT as i32,
        descent: -2 * UNIT as i32,
        ..Font::default()
    };
    let mut c = FIRST;
    while font.glyphs.len() < GLYPHS as usize {
        let Some(ch) = char::from_u32(c) else {
            c += 1;
            continue;
        };
        let mask = Mask {
            width: CELL,
            height: CELL,
            bits: (0..bytes).map(|i| (i as u32 ^ c) as u8).collect(),
        };
        let side = CELL as f32 * UNIT;
        font.chars.push(CharGlyph {
            char: ch as u32,
            glyph: font.glyphs.len() as u32,
        });
        font.glyphs.push(Glyph {
            name: String::new(),
            advance: side as i32,
            ops: vec![Op::Mask(MaskOp {
                rect: Rect::new(0.0, 0.0, side, side),
                mask,
                color: None,
            })],
            entry: None,
            exit: None,
        });
        c += 1;
    }
    font.normalize();
    font
}

fn text(font: &Font) -> String {
    let step = font.chars.len() / LINE;
    font.chars
        .iter()
        .step_by(step)
        .take(LINE)
        .filter_map(|c| char::from_u32(c.char))
        .collect()
}

fn benches(c: &mut Criterion) {
    let font = unifont();
    let bytes = font.to_bytes();
    let line = text(&font);
    c.bench_function("load (57k mask glyphs)", |b| {
        b.iter(|| Font::from_bytes(black_box(&bytes)).unwrap())
    });
    c.bench_function("find a glyph", |b| {
        b.iter(|| {
            line.chars()
                .filter_map(|ch| font.glyph_of(black_box(ch)))
                .count()
        })
    });
    c.bench_function("find a glyph, linear (before)", |b| {
        b.iter(|| {
            line.chars()
                .filter_map(|ch| {
                    font.chars
                        .iter()
                        .find(|m| m.char == black_box(ch) as u32)
                        .map(|m| m.glyph)
                })
                .count()
        })
    });
    c.bench_function("shape a line (200 chars)", |b| {
        b.iter(|| yog_font::shape(&font, black_box(&line)))
    });
    let shaped = yog_font::shape(&font, &line);
    c.bench_function("draw a line (200 chars, 1 px a unit/8)", |b| {
        b.iter(|| yog_font::raster::run(&font, black_box(&shaped), 1.0 / UNIT, Color::WHITE))
    });
}

criterion_group!(font, benches);
criterion_main!(font);
