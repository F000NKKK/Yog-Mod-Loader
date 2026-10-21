//! SVGs rasterized as textures want them.

use super::{rasterize, size};

const SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="20"><rect width="10" height="20" fill="#ff0000" fill-opacity="0.5"/></svg>"##;

#[test]
fn rasterizes_straight_alpha() {
    assert_eq!(size(SQUARE), Some((10.0, 20.0)));
    let px = rasterize(SQUARE, 4, 8).unwrap();
    assert_eq!(px.len(), 4 * 8 * 4);
    assert_eq!(&px[..4], &[255, 0, 0, 128], "not premultiplied");
    assert!(rasterize("not svg", 4, 4).is_none());
}
