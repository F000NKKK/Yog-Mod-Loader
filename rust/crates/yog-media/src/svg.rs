//! SVG documents: their own size, and their pixels at a size — what the
//! runtime draws in game and the IDE's designers show.

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

/// Opaque alpha: a pixel that needs no un-premultiplying.
const OPAQUE: u32 = 255;

fn tree(data: &str) -> Option<Tree> {
    Tree::from_str(data, &Options::default()).ok()
}

/// `data` as straight-alpha RGBA pixels, `w`×`h`.
pub fn rasterize(data: &str, w: u32, h: u32) -> Option<Vec<u8>> {
    let tree = tree(data)?;
    let mut pixmap = Pixmap::new(w, h)?;
    let size = tree.size();
    let t = Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, t, &mut pixmap.as_mut());
    // tiny-skia keeps premultiplied alpha; textures want it straight.
    let mut px = pixmap.take();
    for p in px.as_chunks_mut::<4>().0 {
        let a = p[3] as u32;
        if a > 0 && a < OPAQUE {
            for c in &mut p[..3] {
                *c = ((*c as u32 * OPAQUE + a / 2) / a).min(OPAQUE) as u8;
            }
        }
    }
    Some(px)
}

/// The document's own size in pixels.
pub fn size(data: &str) -> Option<(f32, f32)> {
    tree(data).map(|t| (t.size().width(), t.size().height()))
}

#[cfg(test)]
mod tests;
