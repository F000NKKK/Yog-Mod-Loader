//! The mod's own fonts (`.yfont`) in the designers: a font id of the mod's
//! namespace is its `content/<path>.yfont`, its glyphs drawn by `yog_font`
//! the way the runtime draws them.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;
use yog_sync::{Arc, Mutex};

use yog_graphics::Color;
use yog_ids::Format;

use super::assets::Resources;
use super::font::{Glyph, GlyphBitmap};

/// Pixels a GUI pixel is drawn at, for the designers' zoom.
const OVERSAMPLE: f32 = 4.0;
/// GUI pixels an em is: the game's line height (as the runtime's).
const EM_PX: f32 = 9.0;

pub(super) struct YogProvider {
    font: yog_font::Font,
    chars: HashMap<char, u32>,
    /// Runs drawn whole, by their text.
    runs: Mutex<HashMap<String, Arc<Glyph>>>,
}

/// Runs kept drawn before the cache starts over.
const MAX_RUNS: usize = 1024;

/// The file of the mod's font `id` (`<mod id>:<path>`), if it's the mod's.
fn file(res: &Resources, id: &str) -> Option<PathBuf> {
    let root = res.mod_root.as_deref()?;
    let (ns, path) = yog_ids::split(id);
    (super::project::mod_id(root).as_deref() == Some(ns)).then(|| {
        root.join(yog_ids::project::CONTENT)
            .join(Format::Font.file(path))
    })
}

/// When the mod's font `id` last changed, if it has one.
pub(super) fn changed(res: &Resources, id: &str) -> Option<SystemTime> {
    std::fs::metadata(file(res, id)?)
        .and_then(|m| m.modified())
        .ok()
}

/// The mod's font `id`, if it has one.
pub(super) fn load(res: &Resources, id: &str) -> Option<Arc<YogProvider>> {
    let text = crate::files::read_source(file(res, id)?).ok()?;
    let mut font = yog_data::from_text::<yog_font::Font>(&text).ok()?.value;
    font.normalize();
    let chars = font
        .chars
        .iter()
        .filter_map(|c| Some((char::from_u32(c.char)?, c.glyph)))
        .collect();
    Some(Arc::new(YogProvider {
        font,
        chars,
        runs: Mutex::new(HashMap::new()),
    }))
}

impl YogProvider {
    pub(super) fn chars(&self) -> impl Iterator<Item = char> + '_ {
        self.chars.keys().copied()
    }

    /// GUI pixels a font unit is.
    fn units(&self) -> f32 {
        EM_PX / self.font.units_per_em.max(1) as f32
    }

    pub(super) fn glyph(&self, ch: char, baseline: f32) -> Option<Glyph> {
        let &index = self.chars.get(&ch)?;
        let advance = self.font.glyphs.get(index as usize)?.advance as f32 * self.units();
        let placed =
            yog_font::raster::glyph(&self.font, index, self.units() * OVERSAMPLE, Color::WHITE);
        Some(drawn(placed, advance, baseline))
    }

    /// `text` shaped whole (ligatures, kerning, contextual forms, joined
    /// glyphs) and drawn as one picture, as the runtime draws it.
    pub(super) fn run(&self, text: &str, baseline: f32) -> Arc<Glyph> {
        if let Some(r) = self.runs.lock().get(text) {
            return r.clone();
        }
        let shaped = yog_font::shape(&self.font, text);
        let placed =
            yog_font::raster::run(&self.font, &shaped, self.units() * OVERSAMPLE, Color::WHITE);
        let run = Arc::new(drawn(placed, shaped.advance * self.units(), baseline));
        let mut runs = self.runs.lock();
        if runs.len() >= MAX_RUNS {
            runs.clear();
        }
        runs.insert(text.to_owned(), run.clone());
        run
    }
}

/// A picture drawn from the pen as the designers' glyph, `advance` GUI
/// pixels wide.
fn drawn(placed: yog_graphics::PlacedImage, advance: f32, baseline: f32) -> Glyph {
    let image = placed.image;
    if image.width == 0 || image.height == 0 {
        return Glyph {
            advance,
            shadow_offset: 1.0,
            bold_offset: 1.0,
            bitmap: None,
        };
    }
    // The bitmap starts at the pen: blank columns before a picture that
    // starts right of it, cut ones (its margin) of one left of it.
    let shift = placed.x;
    let w = (image.width as i32 + shift).max(0) as u32;
    let mut rgba = vec![0u8; (w * image.height) as usize * yog_graphics::BYTES_PER_PIXEL];
    for (y, row) in image.pixels().chunks(image.width as usize).enumerate() {
        for (x, p) in row.iter().enumerate() {
            let to = x as i32 + shift;
            if (0..w as i32).contains(&to) {
                let i = (y * w as usize + to as usize) * yog_graphics::BYTES_PER_PIXEL;
                rgba[i..i + yog_graphics::BYTES_PER_PIXEL].copy_from_slice(p);
            }
        }
    }
    let top = baseline + placed.y as f32 / OVERSAMPLE;
    Glyph {
        advance,
        shadow_offset: 1.0,
        bold_offset: 1.0,
        bitmap: Some(GlyphBitmap {
            w,
            h: image.height,
            rgba,
            oversample: OVERSAMPLE,
            top,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::super::font::Font;
    use super::*;
    use yog_graphics::{Image, ImageOp, Op, Rect};

    #[test]
    fn the_mods_font_is_drawn_from_its_file() {
        let dir = std::env::temp_dir().join(format!("yog-font-yog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("content/font")).unwrap();
        std::fs::write(dir.join("yog.toml"), "[mod]\nid = \"mymod\"\n").unwrap();
        // One glyph: a 2×2 GUI-pixel square on the baseline, advance 3 px (9 px an em of 72 units).
        let square = Op::Image(ImageOp {
            rect: Rect::new(0.0, 0.0, 16.0, 16.0),
            image: Image {
                width: 1,
                height: 1,
                rgba: vec![0xFF; 4],
            },
            tinted: true,
        });
        let font = yog_font::Font {
            units_per_em: 72,
            ascent: 56,
            descent: -16,
            glyphs: vec![
                yog_font::Glyph {
                    name: "a".into(),
                    advance: 24,
                    ops: vec![square.clone()],
                    ..Default::default()
                },
                // `aa` is one glyph, 4 px wide: a run is shaped whole.
                yog_font::Glyph {
                    name: "aa".into(),
                    advance: 32,
                    ops: vec![square],
                    ..Default::default()
                },
            ],
            chars: vec![yog_font::CharGlyph {
                char: 'a' as u32,
                glyph: 0,
            }],
            ligatures: vec![yog_font::Ligature {
                glyphs: vec![0, 0],
                glyph: 1,
            }],
            ..Default::default()
        };
        std::fs::write(
            dir.join("content/font/square.yfont"),
            yog_data::to_text(&font),
        )
        .unwrap();
        let res = Resources {
            mod_root: Some(dir.clone()),
            vanilla: None,
        };
        let loaded = Font::load(&res, "mymod:font/square");
        assert!(loaded.is_real());
        let g = loaded.glyph('a');
        assert_eq!(g.advance, 3.0);
        let b = g.bitmap.as_ref().unwrap();
        // Its bottom on the baseline, 7 px below the line's top.
        assert!(
            ((b.top + b.h as f32 / b.oversample) - 7.0).abs() <= 1.0 / b.oversample,
            "{} {}",
            b.top,
            b.h
        );
        assert_eq!(
            loaded.width("aa"),
            4.0,
            "the ligature's advance, not two letters'"
        );
        assert_eq!(loaded.width("aaa"), 7.0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
