//! SVG images: `draw2d_svg` and `.svg` image sources
//! (`mymod:gui/logo.svg`). Rasterized here at the size they're drawn
//! (times [`OVERSAMPLE`], so they stay sharp at any GUI scale), handed to
//! the host as a dynamic texture once per size, and drawn like any other
//! texture.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use jni::JNIEnv;
use yog_abi::YogStr;

use crate::jcall::jcall;

/// Physical pixels rasterized per GUI pixel.
const OVERSAMPLE: f32 = 4.0;
/// Largest side of a rasterized SVG.
const MAX_SIDE: u32 = 2048;
/// Rasterized sizes kept before they're all let go.
const MAX_RASTERS: usize = 256;

struct Cache {
    /// (document hash, width, height) → the texture id it was given.
    rasters: HashMap<(u64, u32, u32), String>,
    /// `.svg` resource id → its document (`None`: missing).
    files: HashMap<String, Option<String>>,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Cache) -> R) -> R {
    let mut c = CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(c.get_or_insert_with(|| Cache { rasters: HashMap::new(), files: HashMap::new() }))
}

fn hash(data: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut h);
    h.finish()
}

fn tree(data: &str) -> Option<resvg::usvg::Tree> {
    resvg::usvg::Tree::from_str(data, &resvg::usvg::Options::default()).ok()
}

/// `data` as RGBA pixels, `w`×`h`.
pub fn rasterize(data: &str, w: u32, h: u32) -> Option<Vec<u8>> {
    let tree = tree(data)?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    let size = tree.size();
    let t = resvg::tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, t, &mut pixmap.as_mut());
    // tiny-skia keeps premultiplied alpha; textures want it straight.
    let mut px = pixmap.take();
    for p in px.as_chunks_mut::<4>().0 {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Some(px)
}

/// The document's own size in pixels.
pub fn size(data: &str) -> Option<(f32, f32)> {
    tree(data).map(|t| (t.size().width(), t.size().height()))
}

/// The document of the `.svg` resource `id` (`ns:textures/x.svg`).
pub fn file(env: &mut JNIEnv, id: &str) -> Option<String> {
    if let Some(hit) = with(|c| c.files.get(id).cloned()) {
        return hit;
    }
    let text: Option<String> =
        unsafe { jcall!(env, "dev/yog/NativeDraw", "readResourceText", [S(YogStr::from_str(id))]) }
            .filter(|t: &String| !t.is_empty());
    with(|c| c.files.insert(id.to_string(), text.clone()));
    text
}

/// Draws `data` into `[x, y, w, h]` GUI pixels.
pub fn draw(env: &mut JNIEnv, data: &str, [x, y, w, h]: [f32; 4], tint: u32) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let side = |v: f32| ((v * OVERSAMPLE).ceil() as u32).clamp(1, MAX_SIDE);
    let (pw, ph) = (side(w), side(h));
    let key = (hash(data), pw, ph);
    let id = match with(|c| c.rasters.get(&key).cloned()) {
        Some(id) => id,
        None => {
            let Some(px) = rasterize(data, pw, ph) else { return };
            if with(|c| c.rasters.len()) >= MAX_RASTERS {
                let old: Vec<String> = with(|c| c.rasters.drain().map(|(_, id)| id).collect());
                for id in old {
                    crate::textures::release(env, &id);
                }
            }
            let id = format!("{}{:x}_{pw}x{ph}", yog_ids::yog::DYNAMIC_SVG_PREFIX, key.0);
            if !crate::textures::register(env, &id, pw, ph, &px) {
                return;
            }
            with(|c| c.rasters.insert(key, id.clone()));
            id
        }
    };
    crate::textures::draw(env, &id, (pw, ph), [x, y, w, h], tint);
}

/// `draw2d_svg` — see `YogGfxApi::draw2d_svg`.
pub(crate) unsafe extern "C" fn gfx_draw2d_svg(data: YogStr, x: f32, y: f32, w: f32, h: f32) {
    if let Some(mut env) = crate::get_env() {
        draw(&mut env, unsafe { data.as_str() }, [x, y, w, h], yog_gfx::Color::WHITE.0);
    }
}

/// `draw2d_entity` — see `YogGfxApi::draw2d_entity`.
pub(crate) unsafe extern "C" fn gfx_draw2d_entity(
    entity: YogStr,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    yaw: f32,
    pitch: f32,
) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    if let Some(mut env) = crate::get_env() {
        let _: Option<()> = unsafe {
            jcall!(
                &mut env,
                "dev/yog/NativeDraw",
                "drawEntity",
                [S(entity), F(x), F(y), F(w), F(h), F(yaw), F(pitch)]
            )
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="20"><rect width="10" height="20" fill="#ff0000" fill-opacity="0.5"/></svg>"##;

    #[test]
    fn rasterizes_straight_alpha() {
        assert_eq!(size(SQUARE), Some((10.0, 20.0)));
        let px = rasterize(SQUARE, 4, 8).unwrap();
        assert_eq!(px.len(), 4 * 8 * 4);
        assert_eq!(&px[..4], &[255, 0, 0, 128], "not premultiplied");
        assert!(rasterize("not svg", 4, 4).is_none());
    }
}
