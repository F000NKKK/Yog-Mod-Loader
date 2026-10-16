//! RGBA images.

use yog_data::Schema;

use crate::color::Color;

/// Pixels, RGBA8 row by row from the top, straight alpha.
#[derive(Schema, Debug, Clone, PartialEq, Default)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    /// A transparent `width × height` image.
    pub fn new(width: u32, height: u32) -> Image {
        Image { width, height, rgba: vec![0; (width * height * 4) as usize] }
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    fn index(&self, x: u32, y: u32) -> usize {
        ((y * self.width + x) * 4) as usize
    }

    pub fn pixel(&self, x: u32, y: u32) -> Color {
        let i = self.index(x, y);
        Color::from_rgba8([self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]])
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, c: Color) {
        let i = self.index(x, y);
        self.rgba[i..i + 4].copy_from_slice(&c.to_rgba8());
    }
}

/// An image and where it goes: its top-left corner `x, y` pixels (`y`
/// down) from the origin it was drawn for — a glyph's from its pen
/// position on the baseline, say.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlacedImage {
    pub image: Image,
    pub x: i32,
    pub y: i32,
}
