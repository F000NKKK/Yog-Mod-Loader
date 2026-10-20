//! RGBA images.

use yog_data::Schema;
use yog_data::with::Raw;

use crate::color::Color;

/// Bytes per pixel: red, green, blue, alpha.
pub const BYTES_PER_PIXEL: usize = 4;

/// One RGBA8 pixel.
pub type Rgba8 = [u8; BYTES_PER_PIXEL];

/// Pixels, RGBA8 row by row from the top, straight alpha.
#[derive(Schema, Debug, Clone, PartialEq, Default)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// Written as bytes, not a list of numbers.
    #[data(with = "Raw")]
    pub rgba: Vec<u8>,
}

impl Image {
    /// A transparent `width × height` image.
    pub fn new(width: u32, height: u32) -> Image {
        Image { width, height, rgba: vec![0; width as usize * height as usize * BYTES_PER_PIXEL] }
    }

    /// The pixels, row by row from the top.
    pub fn pixels(&self) -> &[Rgba8] {
        self.rgba.as_chunks().0
    }

    /// The pixels to change, row by row from the top.
    pub fn pixels_mut(&mut self) -> &mut [Rgba8] {
        self.rgba.as_chunks_mut().0
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Pixel `x, y`'s place in [`Self::pixels`].
    fn index(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }

    pub fn pixel(&self, x: u32, y: u32) -> Color {
        Color::from_rgba8(self.pixels()[self.index(x, y)])
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, c: Color) {
        let i = self.index(x, y);
        self.pixels_mut()[i] = c.to_rgba8();
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
