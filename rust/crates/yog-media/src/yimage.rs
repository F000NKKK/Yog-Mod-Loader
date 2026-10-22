//! `yimage`: a picture of few colors (pixel art: most of a mod's textures)
//! as a `.yog` keeps it — a palette, and each pixel's index in it packed in
//! as few bits as the palette needs. Half the bytes of the same pictures as
//! PNG once the `.yog` is zipped. It keeps its `.png` name; the host hands
//! the game the PNG, made in memory ([`to_png`]), so none is on disk.

use yog_data::Schema;
use yog_data::with::Raw;

use crate::image::{Image, encode_png};
use yog_graphics::BYTES_PER_PIXEL;

/// What a yimage starts with.
pub const MAGIC: &[u8] = yog_ids::yimage::MAGIC.as_bytes();
/// The most colors a palette has; a picture of more stays a PNG.
const MAX_COLORS: usize = 1 << u8::BITS;

/// A picture: its palette (RGBA, a color a `u32`, red the high byte) and
/// each pixel's index, `bits` wide, row by row, the highest bit first.
#[derive(Schema, Debug, Clone, PartialEq)]
struct YImage {
    width: u32,
    height: u32,
    palette: Vec<u32>,
    bits: u8,
    #[data(with = "Raw")]
    pixels: Vec<u8>,
}

/// `img` as a yimage; none when it has more colors than a palette holds.
#[must_use]
pub fn encode(img: &Image) -> Option<Vec<u8>> {
    let colors = img.rgba.as_chunks::<BYTES_PER_PIXEL>().0;
    let mut palette: Vec<u32> = colors.iter().map(|c| u32::from_be_bytes(*c)).collect();
    palette.sort_unstable();
    palette.dedup();
    if palette.len() > MAX_COLORS {
        return None;
    }
    let bits = bits_for(palette.len());
    let mut pixels = vec![0u8; (colors.len() * bits as usize).div_ceil(u8::BITS as usize)];
    for (i, c) in colors.iter().enumerate() {
        let index = palette.binary_search(&u32::from_be_bytes(*c)).ok()? as u32;
        put(&mut pixels, i * bits as usize, bits, index);
    }
    let y = YImage {
        width: img.width,
        height: img.height,
        palette,
        bits,
        pixels,
    };
    let mut out = MAGIC.to_vec();
    out.extend(yog_data::to_bytes(&y));
    Some(out)
}

/// Whether `bytes` are a yimage.
#[must_use]
pub fn is(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

/// The picture a yimage holds.
pub fn decode(bytes: &[u8]) -> Result<Image, String> {
    let body = bytes.strip_prefix(MAGIC).ok_or("not a yimage")?;
    let y: YImage = yog_data::read_bytes(body).map_err(|e| format!("a broken yimage: {e:?}"))?;
    let n = y.width as usize * y.height as usize;
    let fits = (1..=u8::BITS as u8).contains(&y.bits)
        && y.pixels.len() * u8::BITS as usize >= n * y.bits as usize;
    if !fits {
        return Err("a yimage's pixels don't fit its size".into());
    }
    let mut rgba = Vec::with_capacity(n * BYTES_PER_PIXEL);
    for i in 0..n {
        let index = get(&y.pixels, i * y.bits as usize, y.bits) as usize;
        let color = y
            .palette
            .get(index)
            .ok_or("a yimage's pixel past its palette")?;
        rgba.extend(color.to_be_bytes());
    }
    Ok(Image {
        width: y.width,
        height: y.height,
        rgba,
    })
}

/// The PNG of a yimage (what the game reads).
pub fn to_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    encode_png(&decode(bytes)?)
}

/// The bits an index into a palette of `colors` takes (at least one).
fn bits_for(colors: usize) -> u8 {
    (usize::BITS - colors.saturating_sub(1).leading_zeros()).max(1) as u8
}

/// `value`'s low `bits` written from bit `at` on (the highest first).
fn put(out: &mut [u8], at: usize, bits: u8, value: u32) {
    for k in 0..bits as usize {
        if value >> (bits as usize - 1 - k) & 1 == 1 {
            let bit = at + k;
            out[bit / u8::BITS as usize] |= 1 << (u8::BITS as usize - 1 - bit % u8::BITS as usize);
        }
    }
}

/// The `bits` from bit `at` on.
fn get(data: &[u8], at: usize, bits: u8) -> u32 {
    (0..bits as usize).fold(0, |v, k| {
        let bit = at + k;
        let set =
            data[bit / u8::BITS as usize] >> (u8::BITS as usize - 1 - bit % u8::BITS as usize) & 1;
        v << 1 | u32::from(set)
    })
}

#[cfg(test)]
#[path = "yimage_tests.rs"]
mod tests;
