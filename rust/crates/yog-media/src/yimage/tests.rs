use super::*;

/// A picture of `colors` colors, `w × h`.
fn picture(w: u32, h: u32, colors: u32) -> Image {
    let rgba = (0..w * h)
        .flat_map(|i| [(i % colors) as u8, 7, (i % colors / 3) as u8, 0xFF])
        .collect();
    Image {
        width: w,
        height: h,
        rgba,
    }
}

#[test]
fn pictures_round_trip_at_every_width() {
    for colors in [1, 2, 3, 5, 16, 17, 200, 256] {
        let img = picture(16, 9, colors);
        let y = encode(&img).unwrap();
        assert!(is(&y));
        assert_eq!(decode(&y).unwrap(), img, "{colors} colors");
    }
}

#[test]
fn indices_take_as_few_bits_as_the_palette_needs() {
    assert_eq!(
        [1, 2, 3, 4, 5, 16, 17, 256].map(bits_for),
        [1, 1, 2, 2, 3, 4, 5, 8]
    );
    let two = encode(&picture(16, 16, 2)).unwrap();
    // 256 pixels a bit each: 32 bytes of indices, the rest its header (a
    // PNG's is as big) — against 1024 as RGBA.
    const INDICES: usize = 32;
    const HEADER: usize = 64;
    assert!(two.len() <= INDICES + HEADER, "{} bytes", two.len());
}

#[test]
fn too_many_colors_stay_a_png() {
    assert!(encode(&picture(32, 32, 300)).is_none());
}

#[test]
fn its_png_is_the_picture() {
    let img = picture(4, 4, 3);
    let png = to_png(&encode(&img).unwrap()).unwrap();
    assert_eq!(crate::image::decode(&png).unwrap(), img);
    assert!(decode(b"YIMGbroken").is_err() && decode(b"\x89PNG").is_err());
}
