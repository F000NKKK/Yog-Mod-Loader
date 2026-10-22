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
use yog_sync::Mutex;

use jni::JNIEnv;
use yog_abi::YogStr;
use yog_gfx::image::{self, ImageSource, LegacyRegion, Sprite, SpriteSource};
use yog_gfx::{Color, Rect, Vec2};
use yog_ui::ICON_SIZE;

use crate::jcall::jcall;

/// Sprite name → the index of the source the host has (`Sprite::sources`), asked once per sprite.
static RESOLVED: Mutex<Option<HashMap<String, Option<usize>>>> = Mutex::new(None);
/// Texture id → pixel size (`None` = missing), asked once per id.
type SizeCache = HashMap<String, Option<Vec2>>;
static TEXTURE_SIZES: Mutex<Option<SizeCache>> = Mutex::new(None);
/// Sources already reported as unresolvable (logged once each).
static WARNED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn warn_once(source: &str, why: &str) {
    let mut warned = WARNED.lock();
    if warned
        .get_or_insert_with(HashSet::new)
        .insert(source.to_string())
    {
        yog_logging::warn!("image \"{}\": {}", source, why);
    }
}

/// Whether the host's GUI sprite atlas has sprite `id` (none before 1.20.2).
fn has_gui_sprite(env: &mut JNIEnv, id: &str) -> bool {
    unsafe {
        jcall!(
            env,
            "dev/yog/NativeDraw",
            "hasGuiSprite",
            [S(YogStr::from_str(id))]
        )
    }
    .unwrap_or(false)
}

/// The source of `sp` this game has: the first of its sources the host finds.
fn resolve<'s>(env: &mut JNIEnv, sp: &'s Sprite) -> Option<SpriteSource<'s>> {
    let sources = sp.sources();
    let cached = RESOLVED
        .lock()
        .as_ref()
        .and_then(|m| m.get(&sp.name).copied());
    let index = match cached {
        Some(i) => i,
        None => {
            let found = sources.iter().position(|s| match s {
                SpriteSource::Atlas(id) => has_gui_sprite(env, id),
                SpriteSource::Region(r) => texture_size(env, &r.texture).is_some(),
            });
            let mut r = RESOLVED.lock();
            r.get_or_insert_with(HashMap::new)
                .insert(sp.name.clone(), found);
            found
        }
    };
    index.and_then(|i| sources.into_iter().nth(i))
}

/// A sprite's natural size from `source`: an atlas sprite's is the table's, a region's its own.
fn natural_size(sp: &Sprite, source: Option<&SpriteSource<'_>>) -> Vec2 {
    match source {
        Some(SpriteSource::Region(r)) => Vec2::new(r.region.w, r.region.h),
        _ => sp.size,
    }
}

fn texture_size(env: &mut JNIEnv, id: &str) -> Option<Vec2> {
    let mut cache = TEXTURE_SIZES.lock();
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
        (w > 0 && h > 0).then_some(Vec2::new(w as f32, h as f32))
    })();
    cache.insert(id.to_string(), size);
    size
}

/// A region of a texture, in the texture's `texture` pixels.
struct Region {
    uv: Rect,
    texture: Vec2,
}

fn draw_texture(env: &mut JNIEnv, id: &str, dest: Rect, region: Region, tint: Color) {
    let Rect { x, y, w, h } = dest;
    let (
        Rect {
            x: u,
            y: v,
            w: uw,
            h: vh,
        },
        Vec2 { x: tw, y: th },
    ) = (region.uv, region.texture);
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
                I(tint.0 as i32)
            ]
        )
    };
}

fn draw_item(env: &mut JNIEnv, id: &str, block: bool, dest: Rect) {
    let Rect { x, y, w, h } = dest;
    let _: Option<()> = unsafe {
        jcall!(
            env,
            "dev/yog/NativeDraw",
            "drawItemRect",
            [S(YogStr::from_str(id)), Z(block), F(x), F(y), F(w), F(h)]
        )
    };
}

fn draw_atlas_sprite(env: &mut JNIEnv, id: &str, dest: Rect, tint: Color) {
    let Rect { x, y, w, h } = dest;
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
                I(tint.0 as i32)
            ]
        )
    };
}

/// Fills `area` with `color`.
fn rect(env: &mut JNIEnv, area: Rect, color: Color) {
    let (x1, y1, x2, y2) = (area.x, area.y, area.right(), area.bottom());
    let _: Option<()> = unsafe {
        jcall!(
            env,
            "dev/yog/NativeDraw",
            "drawRect",
            [F(x1), F(y1), F(x2), F(y2), I(color.0 as i32)]
        )
    };
}

/// The inventory slot: grey with a dark top-left and a light bottom-right
/// edge, like vanilla's container backgrounds (yog-ui's colours).
fn draw_slot(env: &mut JNIEnv, dest: Rect) {
    for (r, color) in yog_ui::inv_slot_bevel(dest) {
        rect(env, r, color);
    }
}

/// A 1.20.1 texture region, nine-sliced by `border` when stretched.
fn draw_legacy(env: &mut JNIEnv, l: &LegacyRegion, border: f32, dest: Rect, tint: Color) {
    let texture = l.texture_size;
    for (to, uv) in image::nine_slice(dest, l.region, border) {
        draw_texture(env, &l.texture, to, Region { uv, texture }, tint);
    }
}

fn draw_sprite(env: &mut JNIEnv, source: &str, sp: &Sprite, dest: Rect, tint: Color) {
    if sp.is_procedural() {
        draw_slot(env, dest);
        return;
    }
    match resolve(env, sp) {
        Some(SpriteSource::Atlas(id)) => draw_atlas_sprite(env, &id, dest, tint),
        Some(SpriteSource::Region(l)) => draw_legacy(env, l, sp.border, dest, tint),
        None => warn_once(
            source,
            "none of this sprite's sources is in this Minecraft version",
        ),
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
    crate::jni_util::guard("gfx_draw2d_image", || {
        unsafe {
            if w <= 0.0 || h <= 0.0 {
                return;
            }
            let source = source.as_str();
            let Some(mut env) = crate::jni_util::attached_env() else {
                return;
            };
            let (dest, tint) = (Rect::new(x, y, w, h), Color(tint));
            match ImageSource::parse(source) {
                Some(ImageSource::Item(id)) => draw_item(&mut env, &id, false, dest),
                Some(ImageSource::Block(id)) => draw_item(&mut env, &id, true, dest),
                Some(ImageSource::Sprite(name)) => match image::sprite(&name) {
                    Some(sp) => draw_sprite(&mut env, source, sp, dest, tint),
                    // A raw atlas id (`sprite:minecraft:widget/button`).
                    None if name.contains(':') && has_gui_sprite(&mut env, &name) => {
                        draw_atlas_sprite(&mut env, &name, dest, tint);
                    }
                    None => warn_once(source, "unknown sprite name (see yog_gfx::image::SPRITES)"),
                },
                Some(ImageSource::Texture(id)) if id.ends_with(".svg") => {
                    match crate::svg::file(&mut env, &id) {
                        Some(doc) => crate::svg::draw(&mut env, &doc, dest, tint),
                        None => warn_once(source, &format!("SVG {id} not found")),
                    }
                }
                Some(ImageSource::Texture(id)) => match texture_size(&mut env, &id) {
                    Some(size) => {
                        let uv = if uw > 0.0 && vh > 0.0 {
                            Rect::new(u, v, uw, vh)
                        } else {
                            Rect::new(0.0, 0.0, size.x, size.y)
                        };
                        draw_texture(&mut env, &id, dest, Region { uv, texture: size }, tint);
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
    });
}

/// `image_size` — see `YogGfxApi::image_size`.
pub(crate) unsafe extern "C" fn gfx_image_size(source: YogStr, w: *mut f32, h: *mut f32) -> bool {
    crate::jni_util::guarded("gfx_image_size", false, || unsafe {
        let source = source.as_str();
        let size = match ImageSource::parse(source) {
            Some(ImageSource::Item(_) | ImageSource::Block(_)) => {
                Some(Vec2::new(ICON_SIZE, ICON_SIZE))
            }
            Some(ImageSource::Sprite(name)) => match image::sprite(&name) {
                Some(sp) => crate::jni_util::attached_env()
                    .map(|mut env| natural_size(sp, resolve(&mut env, sp).as_ref())),
                None => None,
            },
            Some(ImageSource::Texture(id)) if id.ends_with(".svg") => {
                crate::jni_util::attached_env()
                    .and_then(|mut env| crate::svg::file(&mut env, &id))
                    .and_then(|doc| yog_media::svg::size(&doc))
            }
            Some(ImageSource::Texture(id)) => {
                crate::jni_util::attached_env().and_then(|mut env| texture_size(&mut env, &id))
            }
            Some(ImageSource::ModPath(_)) | None => None,
        };
        match size {
            Some(size) if !w.is_null() && !h.is_null() => {
                *w = size.x;
                *h = size.y;
                true
            }
            _ => false,
        }
    })
}
