//! SVG rasterization → RGBA pixel buffer.

/// Rasterize SVG source to an RGBA8 pixel buffer of the given size.
/// Returns `None` if the SVG is invalid, Pixmap allocation fails,
/// or the `svg` feature is not enabled.
pub fn rasterize(svg_data: &str, w: u32, h: u32) -> Option<Vec<u8>> {
    #[cfg(feature = "svg")]
    {
        // Requires `resvg = "0.41"` in Cargo.toml + feature "svg" enabled.
        use ::resvg::tiny_skia::{Pixmap, Transform};
        use ::resvg::usvg::{Options, Tree};

        let opt = Options::default();
        let tree = Tree::from_str(svg_data, &opt).ok()?;
        let mut pixmap = Pixmap::new(w, h)?;
        let size = tree.size();
        let sx = w as f32 / size.width();
        let sy = h as f32 / size.height();
        resvg::render(&tree, Transform::from_scale(sx, sy), &mut pixmap.as_mut());
        return Some(pixmap.data().to_vec());
    }
    #[cfg(not(feature = "svg"))]
    {
        // Without the `svg` feature, produce a 1×1 transparent placeholder.
        let _ = (svg_data, w, h);
        None
    }
}

/// Simple hash to use as cache key for SVG data.
pub fn svg_hash(data: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut h);
    h.finish()
}
