//! A mod's PNGs as the `.yog` keeps them: each one of few colors (pixel
//! art) as a yimage under its own name — half the bytes once zipped; the
//! host hands the game its PNG, in memory (`yog_media::yimage`).

use yog_media::{image, yimage};

/// `assets`' PNGs a yimage is smaller than once zipped, made yimages. Says
/// what it saved.
pub(super) fn pack(assets: &mut [(String, Vec<u8>)]) {
    let (mut packed, mut before, mut after) = (0usize, 0usize, 0usize);
    for (path, bytes) in assets.iter_mut() {
        if !path.starts_with(yog_ids::pack::ASSETS) || !path.ends_with(yog_ids::res::PNG) {
            continue;
        }
        let Some(y) = image::decode(bytes)
            .ok()
            .and_then(|img| yimage::encode(&img))
        else {
            continue;
        };
        let (png, small) = (zipped(bytes), zipped(&y));
        if small < png {
            packed += 1;
            before += png;
            after += small;
            *bytes = y;
        }
    }
    if packed > 0 {
        eprintln!("    {packed} picture(s) as yimage: {before} → {after} bytes zipped");
    }
}

/// How many bytes `bytes` take in a `.yog` (deflated, as it zips them).
fn zipped(bytes: &[u8]) -> usize {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::FileOptions::<()>::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let written = zip
        .start_file(ENTRY, opts)
        .and_then(|()| Ok(zip.write_all(bytes)?))
        .and_then(|()| zip.finish());
    written.map_or(usize::MAX, |c| c.into_inner().len())
}

/// The one entry [`zipped`] writes.
const ENTRY: &str = "picture";

#[cfg(test)]
#[path = "pictures_tests.rs"]
mod tests;
