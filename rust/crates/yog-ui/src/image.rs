//! Image widgets: natural size (asked from the host, cached) and drawing —
//! fitting the image into the widget per its [`Stretch`] and cutting it to
//! the clip rect.

use std::collections::HashMap;
use std::sync::Mutex;

use yog_gfx::draw2d::{self, Draw2D};
use yog_gfx::image::{self as img, fit, ImageSource, Stretch};
use yog_gfx::{GfxContext, ImageSizeFn};

use crate::layout::Rect;
use crate::widget::Image;

struct Host {
    size: Option<ImageSizeFn>,
    cache: Option<HashMap<String, (f32, f32)>>,
}

static HOST: Mutex<Host> = Mutex::new(Host {
    size: None,
    cache: None,
});

/// Lets image widgets size themselves from the host (texture sizes, the
/// version's sprite sizes). Called by `UiRoot::render`; call it yourself
/// before the first layout to get exact sizes from the very first frame.
pub fn use_host_images(ctx: &GfxContext) {
    if let Ok(mut h) = HOST.lock() {
        h.size = Some(ctx.image_size_fn());
    }
}

/// The size `source` has without a host to ask: 16×16 items, the sprite
/// table's size, 16×16 for the rest.
fn offline_size(source: &str) -> (f32, f32) {
    match ImageSource::parse(source) {
        Some(ImageSource::Sprite(name)) => {
            img::sprite(&name).map_or((16.0, 16.0), |s| s.size(true))
        }
        _ => (16.0, 16.0),
    }
}

/// Natural size of `source` in GUI pixels.
pub fn source_size(source: &str) -> (f32, f32) {
    let Ok(mut h) = HOST.lock() else {
        return offline_size(source);
    };
    let Some(size_fn) = h.size else {
        return offline_size(source);
    };
    let cache = h.cache.get_or_insert_with(HashMap::new);
    if let Some(size) = cache.get(source) {
        return *size;
    }
    match draw2d::image_size_with(size_fn, source) {
        Some(size) => {
            if cache.len() > 4096 {
                cache.clear();
            }
            cache.insert(source.to_owned(), size);
            size
        }
        // Not cached: a texture may only be loadable later (resource reload).
        None => offline_size(source),
    }
}

/// The image's natural size: its region's, or the whole source's.
pub fn natural_size(image: &Image) -> (f32, f32) {
    match image.region {
        Some([_, _, w, h]) if w > 0.0 && h > 0.0 => (w, h),
        _ => source_size(&image.source),
    }
}

/// Item and sprite images can only be drawn whole (no cutting).
fn whole_only(source: &str) -> bool {
    matches!(
        ImageSource::parse(source),
        Some(ImageSource::Item(_) | ImageSource::Block(_) | ImageSource::Sprite(_))
    )
}

/// Draws `image` into the content box `bx`, cut to `clip`.
pub(crate) fn render(d2d: &Draw2D, image: &Image, bx: Rect, clip: Option<Rect>) {
    let (nw, nh) = natural_size(image);
    let whole = whole_only(&image.source);
    let stretch = match image.stretch {
        // Can't be cut: shown whole instead.
        Stretch::UniformToFill if whole => Stretch::Uniform,
        s => s,
    };
    let ([x, y, w, h], [u0, v0, u1, v1]) = fit(stretch, nw, nh, bx.x, bx.y, bx.w, bx.h);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let dest = Rect { x, y, w, h };
    if whole {
        // Drawn whole or not at all — like text lines and item icons.
        let inside = clip.is_none_or(|c| {
            x >= c.x - 0.5 && y >= c.y - 0.5 && x + w <= c.x + c.w + 0.5 && y + h <= c.y + c.h + 0.5
        });
        if inside {
            d2d.image_region(&image.source, x, y, w, h, [0.0; 4], image.tint);
        }
        return;
    }
    // The source region shown, in image pixels.
    let [ru, rv, rw, rh] = match image.region {
        Some(r) if r[2] > 0.0 && r[3] > 0.0 => r,
        _ => [0.0, 0.0, nw, nh],
    };
    let mut src = [ru + u0 * rw, rv + v0 * rh, (u1 - u0) * rw, (v1 - v0) * rh];
    let mut dest = dest;
    if let Some(c) = clip {
        let Some(cut) = dest.intersect(&c) else {
            return;
        };
        // Texture pixels per GUI pixel.
        let (sx, sy) = (src[2] / dest.w, src[3] / dest.h);
        src = [
            src[0] + (cut.x - dest.x) * sx,
            src[1] + (cut.y - dest.y) * sy,
            cut.w * sx,
            cut.h * sy,
        ];
        dest = cut;
    }
    d2d.image_region(
        &image.source,
        dest.x,
        dest.y,
        dest.w,
        dest.h,
        src,
        image.tint,
    );
}
