//! SVG images: `draw2d_svg` and `.svg` image sources
//! (`mymod:gui/logo.svg`). Rasterized here at the size they're drawn
//! (times [`OVERSAMPLE`], so they stay sharp at any GUI scale), handed to
//! the host as a dynamic texture once per size, and drawn like any other
//! texture.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use yog_sync::Mutex;

use jni::JNIEnv;
use yog_abi::YogStr;
use yog_gfx::{Color, Rect};

use crate::jni::call::jcall;

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
    let mut c = CACHE.lock();
    f(c.get_or_insert_with(|| Cache {
        rasters: HashMap::new(),
        files: HashMap::new(),
    }))
}

fn hash(data: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut h);
    h.finish()
}

/// The document of the `.svg` resource `id` (`ns:textures/x.svg`).
pub fn file(env: &mut JNIEnv, id: &str) -> Option<String> {
    if let Some(hit) = with(|c| c.files.get(id).cloned()) {
        return hit;
    }
    let text: Option<String> = unsafe {
        jcall!(
            env,
            "dev/yog/NativeDraw",
            "readResourceText",
            [S(YogStr::from_str(id))]
        )
    }
    .filter(|t: &String| !t.is_empty());
    with(|c| c.files.insert(id.to_string(), text.clone()));
    text
}

/// Draws `data` into `dest` GUI pixels.
pub fn draw(env: &mut JNIEnv, data: &str, dest: Rect, tint: Color) {
    let Rect { w, h, .. } = dest;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let side = |v: f32| ((v * OVERSAMPLE).ceil() as u32).clamp(1, MAX_SIDE);
    let (pw, ph) = (side(w), side(h));
    let key = (hash(data), pw, ph);
    let id = match with(|c| c.rasters.get(&key).cloned()) {
        Some(id) => id,
        None => {
            let Some(px) = yog_media::svg::rasterize(data, pw, ph) else {
                return;
            };
            if with(|c| c.rasters.len()) >= MAX_RASTERS {
                let old: Vec<String> = with(|c| c.rasters.drain().map(|(_, id)| id).collect());
                for id in old {
                    crate::gfx::textures::release(env, &id);
                }
            }
            let id = format!("{}{:x}_{pw}x{ph}", yog_ids::yog::DYNAMIC_SVG_PREFIX, key.0);
            if !crate::gfx::textures::register(env, &id, pw, ph, &px) {
                return;
            }
            with(|c| c.rasters.insert(key, id.clone()));
            id
        }
    };
    crate::gfx::textures::draw(env, &id, (pw, ph), dest, tint);
}

/// `draw2d_svg` — see `YogGfxApi::draw2d_svg`.
pub(crate) unsafe extern "C" fn gfx_draw2d_svg(data: YogStr, x: f32, y: f32, w: f32, h: f32) {
    crate::jni::util::guard("gfx_draw2d_svg", || {
        if let Some(mut env) = crate::jni::util::attached_env() {
            draw(
                &mut env,
                unsafe { data.as_str() },
                Rect::new(x, y, w, h),
                Color::WHITE,
            );
        }
    });
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
    crate::jni::util::guard("gfx_draw2d_entity", || {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        if let Some(mut env) = crate::jni::util::attached_env() {
            let _: Option<()> = unsafe {
                jcall!(
                    &mut env,
                    "dev/yog/NativeDraw",
                    "drawEntity",
                    [S(entity), F(x), F(y), F(w), F(h), F(yaw), F(pitch)]
                )
            };
        }
    });
}
