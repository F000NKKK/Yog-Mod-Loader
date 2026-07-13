//! Custom TTF/OTF font support for book rendering.
//! Uses `fontdue` to rasterize glyphs into a packed atlas texture (RGBA8).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Reference to a registered font — serializable, stored in `BookPage::CustomText`.
/// The actual TTF bytes live in `BookFontRegistry` (registered separately).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookFont {
    /// Font registry ID (e.g. `"hexcasting:display_font"`).
    pub font_id: String,
    /// Render size in pixels (e.g. 14.0).
    pub size_px: f32,
}

/// Global font data registry — maps font_id → raw TTF/OTF bytes.
/// Populated independently of book JSON, before the first render.
#[derive(Debug, Default)]
pub struct BookFontRegistry {
    pub fonts: HashMap<String, Vec<u8>>,
}

impl BookFontRegistry {
    pub fn register(&mut self, id: impl Into<String>, ttf: Vec<u8>) {
        self.fonts.insert(id.into(), ttf);
    }
    pub fn get(&self, id: &str) -> Option<&[u8]> {
        self.fonts.get(id).map(Vec::as_slice)
    }
}

/// UV coordinates + metrics for one glyph in the atlas.
#[derive(Debug, Clone)]
pub struct GlyphInfo {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub width: u32,
    pub height: u32,
    pub xoff: f32,
    pub yoff: f32,
    pub advance: f32,
}

/// Built glyph atlas: RGBA8 pixel buffer + glyph map.
/// Upload `pixels` to GPU via `ctx.create_texture_rgba()` to get a GL handle.
pub struct FontAtlas {
    pub pixels: Vec<u8>,
    pub atlas_size: u32,
    pub glyphs: HashMap<char, GlyphInfo>,
    pub line_height: f32,
}

/// ASCII + common Latin Extended-A charset for the atlas.
#[cfg(feature = "fonts")]
const ATLAS_CHARS: &str = " !\"#$%&'()*+,-./0123456789:;<=>?@\
     ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`\
     abcdefghijklmnopqrstuvwxyz{|}~\
     ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖØÙÚÛÜÝÞß\
     àáâãäåæçèéêëìíîïðñòóôõöøùúûüýþÿ";

impl FontAtlas {
    /// Build a glyph atlas from raw TTF/OTF bytes at the given pixel size.
    /// Returns `None` if the `fonts` feature is not enabled or on parse error.
    pub fn build(font_data: &[u8], size_px: f32) -> Option<FontAtlas> {
        Self::build_impl(font_data, size_px)
    }

    #[cfg(feature = "fonts")]
    fn build_impl(font_data: &[u8], size_px: f32) -> Option<FontAtlas> {
        // Requires `fontdue = "0.8"` in Cargo.toml + feature "fonts" enabled.
        let font = ::fontdue::Font::from_bytes(
            font_data,
            ::fontdue::FontSettings {
                scale: size_px,
                ..Default::default()
            },
        )
        .ok()?;

        const ATLAS_W: u32 = 512;
        const ATLAS_H: u32 = 512;
        let mut pixels = vec![0u8; (ATLAS_W * ATLAS_H * 4) as usize];
        let mut glyphs = HashMap::new();

        let row_h = size_px.ceil() as u32 + 2;
        let mut cx: u32 = 1;
        let mut cy: u32 = 1;

        for ch in ATLAS_CHARS.chars() {
            let (metrics, bitmap) = font.rasterize(ch, size_px);
            let gw = metrics.width as u32;
            let gh = metrics.height as u32;

            if gw == 0 || gh == 0 {
                glyphs.insert(
                    ch,
                    GlyphInfo {
                        u0: 0.0,
                        v0: 0.0,
                        u1: 0.0,
                        v1: 0.0,
                        width: 0,
                        height: 0,
                        xoff: metrics.xmin as f32,
                        yoff: metrics.ymin as f32,
                        advance: metrics.advance_width,
                    },
                );
                continue;
            }
            if cx + gw + 1 >= ATLAS_W {
                cx = 1;
                cy += row_h;
            }
            if cy + gh + 1 >= ATLAS_H {
                break;
            }

            for row in 0..gh {
                for col in 0..gw {
                    let alpha = bitmap[(row * gw + col) as usize];
                    let idx = ((cy + row) * ATLAS_W + (cx + col)) as usize * 4;
                    pixels[idx] = 255;
                    pixels[idx + 1] = 255;
                    pixels[idx + 2] = 255;
                    pixels[idx + 3] = alpha;
                }
            }

            glyphs.insert(
                ch,
                GlyphInfo {
                    u0: cx as f32 / ATLAS_W as f32,
                    v0: cy as f32 / ATLAS_H as f32,
                    u1: (cx + gw) as f32 / ATLAS_W as f32,
                    v1: (cy + gh) as f32 / ATLAS_H as f32,
                    width: gw,
                    height: gh,
                    xoff: metrics.xmin as f32,
                    yoff: metrics.ymin as f32,
                    advance: metrics.advance_width,
                },
            );
            cx += gw + 1;
        }

        Some(FontAtlas {
            pixels,
            atlas_size: ATLAS_W,
            glyphs,
            line_height: size_px * 1.2,
        })
    }

    #[cfg(not(feature = "fonts"))]
    fn build_impl(_font_data: &[u8], _size_px: f32) -> Option<FontAtlas> {
        None
    }

    /// Measure the width of a text string in pixels.
    pub fn measure_width(&self, text: &str) -> f32 {
        text.chars()
            .map(|c| self.glyphs.get(&c).map(|g| g.advance).unwrap_or(0.0))
            .sum()
    }
}
