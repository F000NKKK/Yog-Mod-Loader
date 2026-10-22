//! One-color pictures, a bit a pixel: a pixel font's glyph is one — a
//! thirty-second of the same picture as RGBA.

use yog_data::Schema;
use yog_data::with::Raw;

use crate::color::Color;

use crate::image::{BYTES_PER_PIXEL, Image};

/// `width × height` pixels, a bit each: rows top to bottom, a row's
/// pixels from its first byte's highest bit on, each row whole bytes.
#[derive(Schema, Debug, Clone, PartialEq, Default)]
pub struct Mask {
    pub width: u32,
    pub height: u32,
    #[data(with = "Raw")]
    pub bits: Vec<u8>,
}

impl Mask {
    /// The bytes a row takes.
    pub fn row_bytes(width: u32) -> usize {
        (width as usize).div_ceil(u8::BITS as usize)
    }

    /// Whether pixel (`x`, `y`) is set.
    pub fn get(&self, x: u32, y: u32) -> bool {
        let at = y as usize * Self::row_bytes(self.width) + x as usize / u8::BITS as usize;
        let bit = u8::BITS - 1 - x % u8::BITS;
        self.bits.get(at).is_some_and(|b| b >> bit & 1 == 1)
    }

    /// `image` as a mask and its one color, when it is one: every pixel
    /// clear or opaque, all opaque ones one color (else its colors and
    /// edges keep it an image).
    pub fn of(image: &Image) -> Option<(Mask, Color)> {
        let mut color = None;
        let row = Self::row_bytes(image.width);
        let mut bits = vec![0u8; row * image.height as usize];
        for (i, p) in image.rgba.as_chunks::<BYTES_PER_PIXEL>().0.iter().enumerate() {
            let alpha = p[BYTES_PER_PIXEL - 1];
            if alpha == 0 {
                continue;
            }
            if alpha != u8::MAX || *color.get_or_insert([p[0], p[1], p[2]]) != [p[0], p[1], p[2]] {
                return None;
            }
            let (x, y) = (i % image.width as usize, i / image.width as usize);
            bits[y * row + x / u8::BITS as usize] |=
                1 << (u8::BITS as usize - 1 - x % u8::BITS as usize);
        }
        let [r, g, b] = color.unwrap_or_default();
        let mask = Mask {
            width: image.width,
            height: image.height,
            bits,
        };
        Some((mask, Color::from_rgba8([r, g, b, u8::MAX])))
    }

    /// As an image: set pixels `color`, the rest clear.
    pub fn image(&self, color: Color) -> Image {
        let mut out = Image::new(self.width, self.height);
        let px = color.to_rgba8();
        for y in 0..self.height {
            for x in 0..self.width {
                if self.get(x, y) {
                    let at = (y * self.width + x) as usize * BYTES_PER_PIXEL;
                    out.rgba[at..at + BYTES_PER_PIXEL].copy_from_slice(&px);
                }
            }
        }
        out
    }
}
