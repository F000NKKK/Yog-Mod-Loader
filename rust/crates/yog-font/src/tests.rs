use yog_graphics::{Color, Fill, Op, Seg, Stroke};

use crate::format::*;
use crate::{raster, shape};

fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Seg> {
    vec![Seg::Move { x: x0, y: y0 }, Seg::Line { x: x1, y: y0 }, Seg::Line { x: x1, y: y1 }, Seg::Line { x: x0, y: y1 }, Seg::Close]
}

fn glyph(name: &str, advance: i32, ops: Vec<Op>) -> Glyph {
    Glyph { name: name.into(), advance, ops, entry: None, exit: None }
}

/// `A` a filled box, `B` nothing, `A` `B` one wide glyph.
fn latin() -> Font {
    Font {
        units_per_em: 100,
        ascent: 80,
        descent: -20,
        glyphs: vec![
            glyph("a", 100, vec![Op::Fill(Fill { path: rect(10.0, 0.0, 90.0, 80.0), color: None })]),
            glyph("b", 50, vec![]),
            glyph("a_b", 300, vec![]),
        ],
        chars: vec![CharGlyph { char: 'A' as u32, glyph: 0 }, CharGlyph { char: 'B' as u32, glyph: 1 }],
        ligatures: vec![Ligature { glyphs: vec![0, 1], glyph: 2 }],
        kerning: vec![Kern { left: 0, right: 0, x: -10 }],
        ..Font::default()
    }
}

#[test]
fn characters_become_glyphs_ligatures_and_kerning() {
    let f = latin();
    let s = shape(&f, "AAB?");
    // A, then AB as one glyph kerned... A-A kerning applies to the first pair only.
    assert_eq!(s.glyphs.iter().map(|p| p.glyph).collect::<Vec<_>>(), vec![0, 2]);
    assert_eq!(s.glyphs.iter().map(|p| p.char).collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(s.missing, vec![3]);
    assert_eq!(s.advance, 100.0 + 300.0);
    let k = shape(&f, "AA");
    assert_eq!((k.glyphs[1].x, k.advance), (90.0, 190.0));
}

/// A script drawn stroke by stroke, Hex Casting's runes in small: a start
/// (`>`, heading east), then each character turns the line (`w` straight,
/// `q` left) and draws the next stroke, joined end to end.
fn runes() -> Font {
    let stroke = |name: &str, dx: i32, dy: i32| Glyph {
        name: name.into(),
        advance: 10,
        ops: vec![Op::Stroke(Stroke { path: vec![Seg::Move { x: 0.0, y: 0.0 }, Seg::Line { x: dx as f32, y: dy as f32 }], width: 10.0, color: None })],
        entry: Some([0, 0]),
        exit: Some([dx, dy]),
    };
    // Glyphs 0–3: a stroke heading east, north, west, south; 4, 5: `w`, `q` on their own.
    let mut glyphs = vec![stroke("e", 100, 0), stroke("n", 0, 100), stroke("w", -100, 0), stroke("s", 0, -100)];
    glyphs.push(glyph("letter_w", 60, vec![Op::Fill(Fill { path: rect(0.0, 0.0, 50.0, 50.0), color: None })]));
    glyphs.push(glyph("letter_q", 60, vec![Op::Fill(Fill { path: rect(0.0, 0.0, 50.0, 50.0), color: None })]));
    // States: 0 none, 1 + heading.
    let (start, straight, left) = (1, 2, 3);
    let mut transitions = Vec::new();
    for state in 0..=4 {
        transitions.push(Transition { state, class: start, next: 1, glyph: None });
    }
    for h in 0..4u32 {
        transitions.push(Transition { state: h + 1, class: straight, next: h + 1, glyph: Some(h) });
        let l = (h + 1) % 4;
        transitions.push(Transition { state: h + 1, class: left, next: l + 1, glyph: Some(l) });
    }
    Font {
        units_per_em: 100,
        ascent: 80,
        descent: -20,
        glyphs,
        chars: vec![
            CharGlyph { char: '>' as u32, glyph: 0 },
            CharGlyph { char: 'w' as u32, glyph: 4 },
            CharGlyph { char: 'q' as u32, glyph: 5 },
        ],
        machines: vec![Machine {
            classes: vec![GlyphClass { class: start, glyphs: vec![0] }, GlyphClass { class: straight, glyphs: vec![4] }, GlyphClass { class: left, glyphs: vec![5] }],
            transitions,
        }],
        fit: Some(Fit { height: 60.0, center: 30.0 }),
        ..Font::default()
    }
}

#[test]
fn a_script_of_strokes_turns_and_joins() {
    let f = runes();
    let s = shape(&f, ">wqq");
    // East, east, north, west — each stroke starting where the last ended.
    assert_eq!(s.glyphs.iter().map(|p| p.glyph).collect::<Vec<_>>(), vec![0, 0, 1, 2]);
    let unscaled: Vec<(f32, f32)> = s.glyphs.iter().map(|p| ((p.x - s.glyphs[0].x) / p.scale, p.y / p.scale)).collect();
    let rel: Vec<(f32, f32)> = unscaled.iter().map(|(x, y)| (*x, y - unscaled[0].1)).collect();
    assert_eq!(rel, vec![(0.0, 0.0), (100.0, 0.0), (200.0, 0.0), (200.0, 100.0)]);
    // Taller than the line's 60: scaled to fit, centred on 30.
    let scale = s.glyphs[0].scale;
    assert!((scale - 60.0 / 110.0).abs() < 1e-4, "{scale}");
    let (lo, hi) = (s.glyphs[0].y - 5.0 * scale, s.glyphs[3].y + 5.0 * scale);
    assert!(((lo + hi) / 2.0 - 30.0).abs() < 1e-3, "{lo} {hi}");
    // Its width: the drawing's, plus the last stroke's advance.
    assert!((s.advance - (210.0 * scale + 10.0)).abs() < 1e-3, "{}", s.advance);
}

#[test]
fn without_its_start_a_rune_letter_stands_alone() {
    let f = runes();
    let s = shape(&f, "q>w");
    assert_eq!(s.glyphs.iter().map(|p| p.glyph).collect::<Vec<_>>(), vec![5, 0, 0]);
    assert_eq!(s.glyphs[0].x, 0.0);
    assert!(s.glyphs[1].x >= 60.0);
}

#[test]
fn glyphs_draw_filled_and_stroked() {
    let f = latin();
    let a = raster::glyph(&f, 0, 1.0, Color::WHITE);
    // The box 10..90 × 0..80 units at a pixel a unit, a pixel of margin;
    // its top-left 9 right of the origin and 81 above it.
    assert_eq!((a.x, a.y, a.image.width, a.image.height), (9, -81, 82, 82));
    assert_eq!(a.image.pixel(41, 41), Color::WHITE);
    assert_eq!(a.image.pixel(0, 0).a(), 0);
    let r = runes();
    let s = raster::glyph(&r, 0, 1.0, Color::rgb(0xff0000));
    // A stroke 100 long along the baseline, 10 wide, round-capped.
    let (x, base) = ((50 - s.x) as u32, (-s.y) as u32);
    assert_eq!(s.image.pixel(x, base), Color::rgb(0xff0000));
    // 5 units to each side of the line, then nothing.
    assert_eq!(s.image.pixel(x, base - 4).a(), 255);
    assert_eq!(s.image.pixel(x, 0).a(), 0);
}

#[test]
fn a_run_draws_whole_and_a_font_survives_its_file() {
    let f = runes();
    let s = shape(&f, ">wqq");
    let b = raster::run(&f, &s, 1.0, Color::WHITE);
    assert!(b.image.width > 0 && b.image.rgba.chunks(4).any(|p| p[3] == 255));
    assert_eq!(Font::from_bytes(&f.to_bytes()).unwrap(), f);
}

#[test]
fn invalid_font_references_are_reported() {
    let mut f = latin();
    f.chars[0].glyph = 99;
    f.ligatures[0].glyphs = vec![0];
    let problems = f.validate().unwrap_err();
    assert!(problems.iter().any(|p| p.contains("chars[0].glyph 99")));
    assert!(problems.iter().any(|p| p.contains("at least two glyphs")));
    assert!(Font::from_bytes(&f.to_bytes()).is_err());
}
