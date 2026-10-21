//! Pictures: decoded from any format the build reads, written as the PNG
//! the game takes.

pub use yog_graphics::Image;

/// The picture formats a source can be in, by extension.
pub const EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tga"];

/// Whether `path`'s extension is a picture format [`decode`] reads.
#[must_use]
pub fn is_picture(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// `bytes` (any of [`EXTENSIONS`], told apart by their content) as RGBA.
pub fn decode(bytes: &[u8]) -> Result<Image, String> {
    let rgba = image::load_from_memory(bytes)
        .map_err(|e| format!("not a picture this reads: {e}"))?
        .into_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(Image {
        width,
        height,
        rgba: rgba.into_raw(),
    })
}

/// `img` as a PNG.
pub fn encode_png(img: &Image) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, img.width, img.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer
        .write_image_data(&img.rgba)
        .map_err(|e| e.to_string())?;
    drop(writer);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_round_trips_through_png() {
        let img = Image {
            width: 2,
            height: 1,
            rgba: vec![255, 0, 0, 255, 0, 0, 255, 128],
        };
        let png = encode_png(&img).unwrap();
        assert_eq!(decode(&png).unwrap(), img);
        assert!(is_picture(std::path::Path::new("a/b.PNG")));
        assert!(decode(b"not an image").is_err());
    }
}
