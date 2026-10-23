//! A decoded frame's 8-bit YUV 4:2:0 planes as RGBA: BT.709, limited
//! range (what the build encodes), in fixed point.

/// One plane: its bytes and the bytes from one row to the next.
#[derive(Clone, Copy)]
pub struct Plane<'a> {
    pub bytes: &'a [u8],
    pub stride: usize,
}

/// A frame's planes: luma at full size, the two chromas at half.
#[derive(Clone, Copy)]
pub struct Planes<'a> {
    pub y: Plane<'a>,
    pub u: Plane<'a>,
    pub v: Plane<'a>,
}

/// Fixed point: coefficients scaled by `1 << SHIFT`, rounded at the end.
const SHIFT: u32 = 16;
const ROUND: i32 = 1 << (SHIFT - 1);
/// Limited range: luma from 16, chroma around 128.
const LUMA_FLOOR: i32 = 16;
const CHROMA_ZERO: i32 = 128;
/// BT.709's limited-range coefficients (`255/219` for luma, then the
/// matrix), times `1 << SHIFT`.
const Y_GAIN: i32 = 76_309;
const V_TO_R: i32 = 117_489;
const U_TO_G: i32 = 13_975;
const V_TO_G: i32 = 34_925;
const U_TO_B: i32 = 138_438;
/// Bytes a pixel takes in RGBA, and its opaque alpha.
pub const RGBA: usize = 4;
const OPAQUE: u8 = u8::MAX;
/// Chroma is sampled once per this many pixels each way.
pub(super) const CHROMA_STEP: usize = 2;

fn channel(v: i32) -> u8 {
    ((v + ROUND) >> SHIFT).clamp(0, i32::from(u8::MAX)) as u8
}

/// One pixel from its luma and chroma.
fn pixel(y: u8, u: u8, v: u8) -> [u8; RGBA] {
    let y = (i32::from(y) - LUMA_FLOOR) * Y_GAIN;
    let (u, v) = (i32::from(u) - CHROMA_ZERO, i32::from(v) - CHROMA_ZERO);
    [
        channel(y + V_TO_R * v),
        channel(y - U_TO_G * u - V_TO_G * v),
        channel(y + U_TO_B * u),
        OPAQUE,
    ]
}

/// The top-left `width`×`height` of `planes` written into `out` as RGBA
/// (resized to fit).
pub fn to_rgba(planes: Planes, (width, height): (usize, usize), out: &mut Vec<u8>) {
    out.resize(width * height * RGBA, 0);
    for (row, line) in out.chunks_exact_mut(width * RGBA).enumerate() {
        let ys = &planes.y.bytes[row * planes.y.stride..];
        let crow = row / CHROMA_STEP;
        let us = &planes.u.bytes[crow * planes.u.stride..];
        let vs = &planes.v.bytes[crow * planes.v.stride..];
        for (col, px) in line.as_chunks_mut::<RGBA>().0.iter_mut().enumerate() {
            let c = col / CHROMA_STEP;
            *px = pixel(ys[col], us[c], vs[c]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limited_range_black_white_and_red() {
        assert_eq!(pixel(16, 128, 128), [0, 0, 0, 255]);
        assert_eq!(pixel(235, 128, 128), [255, 255, 255, 255]);
        // BT.709 red, limited range.
        let red = pixel(63, 102, 240);
        assert!(red[0] > 250 && red[1] < 5 && red[2] < 5, "{red:?}");
    }

    #[test]
    fn chroma_is_shared_by_two_by_two_pixels_and_the_frame_is_cropped() {
        let y = [235u8, 16, 16, 0, 16, 235, 16, 0];
        let u = [128u8, 128];
        let v = [128u8, 128];
        let planes = Planes {
            y: Plane {
                bytes: &y,
                stride: 4,
            },
            u: Plane {
                bytes: &u,
                stride: 2,
            },
            v: Plane {
                bytes: &v,
                stride: 2,
            },
        };
        let mut out = Vec::new();
        to_rgba(planes, (3, 2), &mut out);
        let grey: Vec<u8> = out.chunks(RGBA).map(|p| p[0]).collect();
        assert_eq!(grey, vec![255, 0, 0, 0, 255, 0]);
    }
}
