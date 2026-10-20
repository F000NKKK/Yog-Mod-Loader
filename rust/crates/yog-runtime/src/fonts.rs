//! Yog's own fonts (`.yfont`, content of kind `font`): text in them is
//! drawn here — shaped and rasterized by `yog_font` at the screen's
//! resolution, white (the color is the draw's tint), kept as a host
//! texture per run, drawn as an image. Text in any other font is the game's.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use jni::JNIEnv;
use yog_font::Font;
use yog_graphics::Color;
use yog_sync::{Locked, ReadWrite};

/// GUI pixels an em is: the game's line height.
const EM: f32 = 9.0;
/// Where a text's shadow goes, in GUI pixels (times its scale), as the game draws it.
const SHADOW_OFFSET: f32 = 1.0;
/// A shadow's color is the text's, each channel divided by this (the game's).
const SHADOW_DIM: u32 = 4;
/// Rasterized runs kept before they're all let go.
const MAX_RUNS: usize = 512;

static FONTS: RwLock<Option<HashMap<String, Arc<Font>>>> = RwLock::new(None);

/// A run rasterized: its texture, its pixel size, its top-left from the
/// pen position on the baseline (pixels, `y` down).
#[derive(Clone)]
struct Run {
    texture: String,
    size: (u32, u32),
    offset: (i32, i32),
}

#[derive(Default)]
struct Runs {
    /// (font, text, pixels per em) → its run.
    by_key: HashMap<(String, String, u32), Run>,
    /// Textures made so far: the next one's number.
    made: u64,
}

static RUNS: Mutex<Option<Runs>> = Mutex::new(None);

/// Keeps font `id` (a `.yfont`).
pub(crate) fn register(id: String, font: Font) {
    FONTS.write_locked().get_or_insert_with(HashMap::new).insert(id, Arc::new(font));
}

/// Yog's font `id`, if it is one.
pub(crate) fn get(id: &str) -> Option<Arc<Font>> {
    FONTS.read_locked().as_ref()?.get(id).cloned()
}

/// GUI pixels a font unit is, at `scale`.
fn unit(font: &Font, scale: f32) -> f32 {
    EM * scale / font.units_per_em.max(1) as f32
}

/// How wide `text` is in `font` at `scale`, GUI pixels.
pub(crate) fn width(font: &Font, text: &str, scale: f32) -> f32 {
    yog_font::shape(font, text).advance * unit(font, scale)
}

/// `text`'s run in font `id` at `px_per_em` pixels to an em: made (and kept) once.
fn run(env: &mut JNIEnv, id: &str, font: &Font, text: &str, px_per_em: u32) -> Option<Run> {
    let key = (id.to_owned(), text.to_owned(), px_per_em);
    let mut runs = RUNS.locked();
    let runs = runs.get_or_insert_with(Runs::default);
    if let Some(r) = runs.by_key.get(&key) {
        return Some(r.clone());
    }
    let shaped = yog_font::shape(font, text);
    let placed = yog_font::raster::run(font, &shaped, px_per_em as f32 / font.units_per_em.max(1) as f32, Color::WHITE);
    if placed.image.is_empty() {
        return None;
    }
    if runs.by_key.len() >= MAX_RUNS {
        for (_, r) in runs.by_key.drain() {
            crate::textures::release(env, &r.texture);
        }
    }
    runs.made += 1;
    let texture = format!("{}{}", yog_ids::yog::DYNAMIC_TEXT_PREFIX, runs.made);
    let size = (placed.image.width, placed.image.height);
    if !crate::textures::register(env, &texture, size.0, size.1, &placed.image.rgba) {
        return None;
    }
    let r = Run { texture, size, offset: (placed.x, placed.y) };
    runs.by_key.insert(key, r.clone());
    Some(r)
}

/// `color` (`0xAARRGGBB`, alpha 0 meaning opaque as the game reads it) as a draw's tint.
fn tint(color: u32) -> u32 {
    const ALPHA: u32 = 0xFF00_0000;
    if color & ALPHA == 0 { color | ALPHA } else { color }
}

/// The shadow's tint for `tint`: each color channel dimmed, alpha kept.
fn shadow(tint: u32) -> u32 {
    const CHANNEL: u32 = 0xFF;
    let dim = |shift: u32| ((tint >> shift & CHANNEL) / SHADOW_DIM) << shift;
    (tint & 0xFF00_0000) | dim(16) | dim(8) | dim(0)
}

/// Draws `text` in font `id` with its line's top-left at `x, y` GUI pixels.
pub(crate) fn draw(id: &str, font: &Font, text: &str, [x, y, scale]: [f32; 3], color: u32, with_shadow: bool) {
    let Some(mut env) = crate::get_env() else { return };
    let sf = crate::gfx_frame::scale_factor();
    let px_per_em = (EM * scale * sf).round().max(1.0) as u32;
    let Some(r) = run(&mut env, id, font, text, px_per_em) else { return };
    let baseline = y + font.ascent as f32 * unit(font, scale);
    let rect = |dx: f32| {
        let (w, h) = (r.size.0 as f32 / sf, r.size.1 as f32 / sf);
        [x + dx + r.offset.0 as f32 / sf, baseline + dx + r.offset.1 as f32 / sf, w, h]
    };
    let tint = tint(color);
    if with_shadow {
        crate::textures::draw(&mut env, &r.texture, r.size, rect(SHADOW_OFFSET * scale), shadow(tint));
    }
    crate::textures::draw(&mut env, &r.texture, r.size, rect(0.0), tint);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two glyphs of 500 units, an em of 1000.
    fn font() -> Font {
        let glyph = yog_font::Glyph { advance: 500, ..Default::default() };
        Font {
            units_per_em: 1000,
            ascent: 800,
            descent: -200,
            line_gap: 0,
            glyphs: vec![glyph.clone(), glyph],
            chars: vec![yog_font::CharGlyph { char: 'a' as u32, glyph: 0 }, yog_font::CharGlyph { char: 'b' as u32, glyph: 1 }],
            ligatures: Vec::new(),
            machines: Vec::new(),
            kerning: Vec::new(),
            fit: None,
        }
    }

    #[test]
    fn a_registered_font_is_ours_and_measures_in_ems() {
        register("test:mono".into(), font());
        let f = get("test:mono").expect("registered");
        assert!(get("minecraft:default").is_none(), "the game's fonts stay the game's");
        assert_eq!(width(&f, "ab", 1.0), EM);
        assert_eq!(width(&f, "ab", 2.0), 2.0 * EM);
    }

    #[test]
    fn a_shadow_is_the_color_dimmed_its_alpha_kept() {
        assert_eq!(shadow(0xFF_FF_80_40), 0xFF_3F_20_10);
        assert_eq!(tint(0x00_12_34_56), 0xFF_12_34_56, "alpha 0 is opaque, as the game reads it");
    }
}
