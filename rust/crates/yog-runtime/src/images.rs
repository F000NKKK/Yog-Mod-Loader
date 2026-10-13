//! `draw2d_image` / `image_size`: Yog image sources (`item:apple`,
//! `sprite:button`, `mymod:gui/logo`, ... — see `yog_gfx::image`) resolved
//! here, on the host side, for the Minecraft version the loader runs on, so a
//! mod binary draws the same picture on every version and loader.
//!
//! Drawing goes through the host's `NativeDraw`: `drawItemRect` (the item
//! renderer), `drawGuiSprite` (the GUI sprite atlas, 1.20.2+) and
//! `drawImage` (a scaled, tinted texture region). On a host without the
//! sprite atlas (1.20.1) sprites come from their `textures/gui/*.png`
//! regions, nine-sliced here when stretched.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

use jni::JNIEnv;
use yog_abi::YogStr;
use yog_gfx::image::{self, ImageSource, LegacyRegion, Sprite};

use crate::jcall::jcall;

/// 0 = not asked yet, 1 = the host has the GUI sprite atlas, 2 = it doesn't.
static ATLAS: AtomicU8 = AtomicU8::new(0);
/// Texture id → pixel size (`None` = missing), asked once per id.
type SizeCache = HashMap<String, Option<(f32, f32)>>;
static TEXTURE_SIZES: Mutex<Option<SizeCache>> = Mutex::new(None);
/// Sources already reported as unresolvable (logged once each).
static WARNED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn warn_once(source: &str, why: &str) {
    let mut warned = WARNED.lock().unwrap_or_else(|e| e.into_inner());
    if warned
        .get_or_insert_with(HashSet::new)
        .insert(source.to_string())
    {
        yog_logging::warn!("image \"{}\": {}", source, why);
    }
}

fn has_atlas(env: &mut JNIEnv) -> bool {
    match ATLAS.load(Ordering::Relaxed) {
        1 => return true,
        2 => return false,
        _ => {}
    }
    let yes = unsafe { jcall!(env, "dev/yog/NativeDraw", "hasGuiSprites", []) }.unwrap_or(false);
    ATLAS.store(if yes { 1 } else { 2 }, Ordering::Relaxed);
    yes
}

fn texture_size(env: &mut JNIEnv, id: &str) -> Option<(f32, f32)> {
    let mut cache = TEXTURE_SIZES.lock().unwrap_or_else(|e| e.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(hit) = cache.get(id) {
        return *hit;
    }
    let size = (|| {
        let packed: i32 = unsafe {
            jcall!(
                env,
                "dev/yog/NativeDraw",
                "textureSize",
                [S(YogStr::from_str(id))]
            )
        }?;
        let (w, h) = ((packed >> 16) & 0xFFFF, packed & 0xFFFF);
        (w > 0 && h > 0).then_some((w as f32, h as f32))
    })();
    cache.insert(id.to_string(), size);
    size
}

#[allow(clippy::too_many_arguments)]
fn draw_texture(
    env: &mut JNIEnv,
    id: &str,
    [x, y, w, h]: [f32; 4],
    [u, v, uw, vh]: [f32; 4],
    (tw, th): (f32, f32),
    tint: u32,
) {
    if w <= 0.0 || h <= 0.0 || uw <= 0.0 || vh <= 0.0 {
        return;
    }
    let _: Option<()> = unsafe {
        jcall!(
            env,
            "dev/yog/NativeDraw",
            "drawImage",
            [
                S(YogStr::from_str(id)),
                F(x),
                F(y),
                F(w),
                F(h),
                F(u),
                F(v),
                F(uw),
                F(vh),
                F(tw),
                F(th),
                I(tint as i32)
            ]
        )
    };
}

fn draw_item(env: &mut JNIEnv, id: &str, block: bool, [x, y, w, h]: [f32; 4]) {
    let _: Option<()> = unsafe {
        jcall!(
            env,
            "dev/yog/NativeDraw",
            "drawItemRect",
            [S(YogStr::from_str(id)), Z(block), F(x), F(y), F(w), F(h)]
        )
    };
}

fn draw_atlas_sprite(env: &mut JNIEnv, id: &str, [x, y, w, h]: [f32; 4], tint: u32) {
    let _: Option<()> = unsafe {
        jcall!(
            env,
            "dev/yog/NativeDraw",
            "drawGuiSprite",
            [
                S(YogStr::from_str(id)),
                F(x),
                F(y),
                F(w),
                F(h),
                I(tint as i32)
            ]
        )
    };
}

fn rect(env: &mut JNIEnv, x1: f32, y1: f32, x2: f32, y2: f32, color: u32) {
    let _: Option<()> = unsafe {
        jcall!(
            env,
            "dev/yog/NativeDraw",
            "drawRect",
            [F(x1), F(y1), F(x2), F(y2), I(color as i32)]
        )
    };
}

/// The inventory slot: grey with a dark top-left and a light bottom-right
/// edge, like vanilla's container backgrounds.
fn draw_slot(env: &mut JNIEnv, [x, y, w, h]: [f32; 4]) {
    rect(env, x, y, x + w, y + h, 0xFF_8B8B8B);
    rect(env, x, y, x + w, y + 1.0, 0xFF_373737);
    rect(env, x, y, x + 1.0, y + h, 0xFF_373737);
    rect(env, x, y + h - 1.0, x + w, y + h, 0xFF_FFFFFF);
    rect(env, x + w - 1.0, y, x + w, y + h, 0xFF_FFFFFF);
}

/// A 1.20.1 texture region, nine-sliced by `border` when stretched.
fn draw_legacy(env: &mut JNIEnv, l: LegacyRegion, border: f32, dest: [f32; 4], tint: u32) {
    let [x, y, w, h] = dest;
    let tex = (l.tex_w, l.tex_h);
    let b = border
        .min(l.w / 2.0)
        .min(l.h / 2.0)
        .min(w / 2.0)
        .min(h / 2.0);
    if b <= 0.0 || (w == l.w && h == l.h) {
        draw_texture(env, l.texture, dest, [l.u, l.v, l.w, l.h], tex, tint);
        return;
    }
    // Columns/rows: (dest pos, dest size, src pos, src size).
    let cols = [
        (x, b, l.u, b),
        (x + b, w - 2.0 * b, l.u + b, l.w - 2.0 * b),
        (x + w - b, b, l.u + l.w - b, b),
    ];
    let rows = [
        (y, b, l.v, b),
        (y + b, h - 2.0 * b, l.v + b, l.h - 2.0 * b),
        (y + h - b, b, l.v + l.h - b, b),
    ];
    for (dy, dh, sv, sh) in rows {
        for (dx, dw, su, sw) in cols {
            draw_texture(
                env,
                l.texture,
                [dx, dy, dw, dh],
                [su, sv, sw, sh],
                tex,
                tint,
            );
        }
    }
}

fn draw_sprite(env: &mut JNIEnv, source: &str, sp: &Sprite, dest: [f32; 4], tint: u32) {
    if sp.is_procedural() {
        draw_slot(env, dest);
    } else if let (true, Some(atlas)) = (has_atlas(env), sp.atlas) {
        draw_atlas_sprite(env, &format!("minecraft:{atlas}"), dest, tint);
    } else if let Some(l) = sp.legacy {
        draw_legacy(env, l, sp.border, dest, tint);
    } else {
        warn_once(
            source,
            "this sprite doesn't exist on this Minecraft version",
        );
    }
}

/// `draw2d_image` — see `YogGfxApi::draw2d_image`.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe extern "C" fn gfx_draw2d_image(
    source: YogStr,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    u: f32,
    v: f32,
    uw: f32,
    vh: f32,
    tint: u32,
) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let source = source.as_str();
    let Some(mut env) = crate::get_env() else {
        return;
    };
    let dest = [x, y, w, h];
    match ImageSource::parse(source) {
        Some(ImageSource::Item(id)) => draw_item(&mut env, &id, false, dest),
        Some(ImageSource::Block(id)) => draw_item(&mut env, &id, true, dest),
        Some(ImageSource::Sprite(name)) => match image::sprite(&name) {
            Some(sp) => draw_sprite(&mut env, source, sp, dest, tint),
            // A raw atlas id (`sprite:minecraft:widget/button`).
            None if name.contains(':') && has_atlas(&mut env) => {
                draw_atlas_sprite(&mut env, &name, dest, tint)
            }
            None => warn_once(source, "unknown sprite name (see yog_gfx::image::SPRITES)"),
        },
        Some(ImageSource::Texture(id)) => match texture_size(&mut env, &id) {
            Some(size) => {
                let region = if uw > 0.0 && vh > 0.0 {
                    [u, v, uw, vh]
                } else {
                    [0.0, 0.0, size.0, size.1]
                };
                draw_texture(&mut env, &id, dest, region, size, tint);
            }
            None => warn_once(source, &format!("texture {id} not found")),
        },
        Some(ImageSource::ModPath(_)) => warn_once(
            source,
            "`mod:` is resolved by `yog build` in .yui files — in Rust write `<mod id>:<path>`",
        ),
        None => warn_once(
            source,
            "not an image source (item:, block:, sprite:, <ns>:<path>)",
        ),
    }
}

/// `image_size` — see `YogGfxApi::image_size`.
pub(crate) unsafe extern "C" fn gfx_image_size(source: YogStr, w: *mut f32, h: *mut f32) -> bool {
    let source = source.as_str();
    let size = match ImageSource::parse(source) {
        Some(ImageSource::Item(_) | ImageSource::Block(_)) => Some((16.0, 16.0)),
        Some(ImageSource::Sprite(name)) => match image::sprite(&name) {
            Some(sp) => crate::get_env().map(|mut env| sp.size(has_atlas(&mut env))),
            None => None,
        },
        Some(ImageSource::Texture(id)) => {
            crate::get_env().and_then(|mut env| texture_size(&mut env, &id))
        }
        Some(ImageSource::ModPath(_)) | None => None,
    };
    match size {
        Some((sw, sh)) if !w.is_null() && !h.is_null() => {
            *w = sw;
            *h = sh;
            true
        }
        _ => false,
    }
}
