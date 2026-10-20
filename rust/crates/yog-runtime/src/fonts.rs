//! Yog's own fonts (`.yfont`, content of kind `font`), under one id for
//! Yog and the game: text a mod draws in one is shaped and rasterized here
//! by `yog_font` at the screen's resolution, white (the color is the
//! draw's tint), kept as a host texture per run and drawn as an image; the
//! game's own text (chat, signs, tooltips) gets the font as a bitmap atlas
//! ([`atlases`]) the host makes its version's font JSON of. The game's
//! own fonts are named only through Yog's aliases (`yog:font/uniform`,
//! [`named`]).

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
/// Fonts named that are neither Yog's nor aliases, reported once each.
static UNKNOWN: Mutex<std::collections::BTreeSet<String>> = Mutex::new(std::collections::BTreeSet::new());

/// Pixels an em is in a font's atlas for the game (it scales them to [`EM`]).
const ATLAS_EM_PX: f32 = 32.0;
/// Glyph cells in a row of an atlas.
const ATLAS_COLUMNS: usize = 16;
/// What fills an atlas's unused cells (the game skips it).
const NO_CHAR: char = '\0';

/// A font as the game's text takes it: one cell per character.
pub(crate) struct Atlas {
    pub(crate) id: String,
    /// The cells, as a PNG.
    pub(crate) png: Vec<u8>,
    /// Each row's characters, cell by cell.
    pub(crate) rows: Vec<String>,
    /// GUI pixels from the top of a cell to the baseline, and a cell's height.
    pub(crate) ascent: i32,
    pub(crate) height: i32,
}

/// Every font's atlas.
pub(crate) fn atlases() -> Vec<Atlas> {
    let fonts: Vec<(String, Arc<Font>)> = FONTS.read_locked().as_ref().map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    fonts.into_iter().filter_map(|(id, f)| atlas(id, &f)).collect()
}

/// Font `font`'s atlas: each of its characters' glyph in a cell of the
/// em's height, its baseline at the ascent.
fn atlas(id: String, font: &Font) -> Option<Atlas> {
    let upem = font.units_per_em.max(1) as f32;
    let px = ATLAS_EM_PX / upem;
    let cell_h = ((font.ascent - font.descent) as f32 * px).ceil().max(1.0) as u32;
    let advance = font.glyphs.iter().map(|g| g.advance).max().unwrap_or(0);
    let cell_w = (advance as f32 * px).ceil().max(1.0) as u32;
    let baseline = (font.ascent as f32 * px).round() as i32;
    let chars: Vec<(char, u32)> = font.chars.iter().filter_map(|c| Some((char::from_u32(c.char)?, c.glyph))).collect();
    if chars.is_empty() {
        return None;
    }
    let rows = chars.len().div_ceil(ATLAS_COLUMNS);
    let mut image = yog_graphics::Image::new(cell_w * ATLAS_COLUMNS as u32, cell_h * rows as u32);
    let mut names = vec![String::new(); rows];
    for (i, (c, glyph)) in chars.iter().enumerate() {
        let (col, row) = ((i % ATLAS_COLUMNS) as u32, (i / ATLAS_COLUMNS) as u32);
        names[row as usize].push(*c);
        let g = yog_font::raster::glyph(font, *glyph, px, Color::WHITE);
        blit(&mut image, &g.image, (col * cell_w) as i32 + g.x, (row * cell_h) as i32 + baseline + g.y, [col * cell_w, row * cell_h, cell_w, cell_h]);
    }
    for row in &mut names {
        while row.chars().count() < ATLAS_COLUMNS {
            row.push(NO_CHAR);
        }
    }
    let png = yog_media::image::encode_png(&image).ok()?;
    let gui = |units: i32| (units as f32 / upem * EM).round() as i32;
    Some(Atlas { id, png, rows: names, ascent: gui(font.ascent), height: gui(font.ascent - font.descent) })
}

/// Copies `src` into `dst` at `x, y`, inside the cell `[cx, cy, cw, ch]` only.
fn blit(dst: &mut yog_graphics::Image, src: &yog_graphics::Image, x: i32, y: i32, [cx, cy, cw, ch]: [u32; 4]) {
    let width = dst.width;
    let pixels = dst.pixels_mut();
    for (sy, row) in src.pixels().chunks(src.width.max(1) as usize).enumerate() {
        for (sx, p) in row.iter().enumerate() {
            let (px, py) = (x + sx as i32, y + sy as i32);
            let inside = px >= cx as i32 && py >= cy as i32 && px < (cx + cw) as i32 && py < (cy + ch) as i32;
            if inside {
                pixels[py as usize * width as usize + px as usize] = *p;
            }
        }
    }
}

/// Keeps font `id` (a `.yfont`).
pub(crate) fn register(id: String, font: Font) {
    FONTS.write_locked().get_or_insert_with(HashMap::new).insert(id, Arc::new(font));
}

/// Yog's font `id`, if it is one.
pub(crate) fn get(id: &str) -> Option<Arc<Font>> {
    FONTS.read_locked().as_ref()?.get(id).cloned()
}

/// What a font a mod names is drawn with.
pub(crate) enum Named {
    /// One of Yog's fonts.
    Yog(Arc<Font>),
    /// The game's font of this id (Yog's alias of it named it).
    Game(String),
}

/// The font `id` names: a Yog font, or the game's one behind a `yog:font/`
/// alias — the game's fonts are never named directly, so anything else
/// is reported once and drawn in the default font.
pub(crate) fn named(id: &str) -> Named {
    if let Some(font) = get(id) {
        return Named::Yog(font);
    }
    if id.is_empty() {
        return Named::Game(yog_ids::game::DEFAULT_FONT.to_owned());
    }
    if let Some(game) = yog_ids::game_font_of(id) {
        return Named::Game(game);
    }
    if UNKNOWN.locked().insert(id.to_owned()) {
        yog_logging::warn!("font {id} is neither a Yog font nor a {}<name> alias; drawn in the default font", yog_ids::yog::FONT_ALIAS);
    }
    Named::Game(yog_ids::game::DEFAULT_FONT.to_owned())
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

/// A colour's (`0xAARRGGBB`) alpha, a channel's bits, and where red, green
/// and blue sit.
const ALPHA: u32 = 0xFF00_0000;
const CHANNEL: u32 = 0xFF;
const RED: u32 = 16;
const GREEN: u32 = 8;
const BLUE: u32 = 0;

/// `color` (`0xAARRGGBB`, alpha 0 meaning opaque as the game reads it) as a draw's tint.
fn tint(color: u32) -> u32 {
    if color & ALPHA == 0 { color | ALPHA } else { color }
}

/// The shadow's tint for `tint`: each color channel dimmed, alpha kept.
fn shadow(tint: u32) -> u32 {
    let dim = |shift: u32| ((tint >> shift & CHANNEL) / SHADOW_DIM) << shift;
    (tint & ALPHA) | dim(RED) | dim(GREEN) | dim(BLUE)
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
    fn a_font_s_atlas_has_a_cell_per_character() {
        let a = atlas("test:mono".into(), &font()).expect("an atlas");
        assert_eq!(a.rows.len(), 1);
        assert!(a.rows[0].starts_with("ab") && a.rows[0].chars().count() == ATLAS_COLUMNS);
        assert_eq!((a.ascent, a.height), ((0.8 * EM).round() as i32, EM as i32));
        assert!(a.png.starts_with(b"\x89PNG"));
    }

    #[test]
    fn a_shadow_is_the_color_dimmed_its_alpha_kept() {
        assert_eq!(shadow(0xFF_FF_80_40), 0xFF_3F_20_10);
        assert_eq!(tint(0x00_12_34_56), 0xFF_12_34_56, "alpha 0 is opaque, as the game reads it");
    }
}
