//! Image widgets: natural size (asked from the host, cached) and drawing —
//! fitting the image into the widget per its [`Stretch`] and cutting it to
//! the clip rect.

use std::collections::HashMap;
use yog_sync::Mutex;

use yog_gfx::draw2d::{self, Draw2D};
use yog_gfx::image::{self as img, ImageSource, Stretch, fit};
use yog_gfx::{GfxContext, ImageSizeFn, Rect, Vec2};

use crate::widget::Image;

struct Host {
    size: Option<ImageSizeFn>,
    cache: Option<HashMap<String, Vec2>>,
}

static HOST: Mutex<Host> = Mutex::new(Host {
    size: None,
    cache: None,
});

/// Lets image widgets size themselves from the host (texture sizes, the
/// version's sprite sizes). Called by `UiRoot::render`; call it yourself
/// before the first layout to get exact sizes from the very first frame.
pub fn use_host_images(ctx: &GfxContext) {
    HOST.lock().size = Some(ctx.image_size_fn());
}

/// The size of an image the host can't tell (an item's, offline).
const FALLBACK_SIZE: Vec2 = Vec2::new(crate::ICON_SIZE, crate::ICON_SIZE);
/// Most image sizes cached before the cache starts over.
const SIZE_CACHE_CAP: usize = 4096;

/// The size `source` has without a host to ask: 16×16 items, the sprite
/// table's size, 16×16 for the rest.
fn offline_size(source: &str) -> Vec2 {
    match ImageSource::parse(source) {
        Some(ImageSource::Sprite(name)) => {
            img::sprite(&name).map_or(FALLBACK_SIZE, |s| s.size(true))
        }
        _ => FALLBACK_SIZE,
    }
}

/// Natural size of `source` in GUI pixels.
pub fn source_size(source: &str) -> Vec2 {
    let mut h = HOST.lock();
    let Some(size_fn) = h.size else {
        return offline_size(source);
    };
    let cache = h.cache.get_or_insert_with(HashMap::new);
    if let Some(size) = cache.get(source) {
        return *size;
    }
    match draw2d::image_size_with(size_fn, source) {
        Some(size) => {
            if cache.len() > SIZE_CACHE_CAP {
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
pub fn natural_size(image: &Image) -> Vec2 {
    match image.region {
        Some(r) if r.w > 0.0 && r.h > 0.0 => Vec2::new(r.w, r.h),
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
    let natural = natural_size(image);
    let whole = whole_only(&image.source);
    let stretch = match image.stretch {
        // Can't be cut: shown whole instead.
        Stretch::UniformToFill if whole => Stretch::Uniform,
        s => s,
    };
    let (dest, shown) = fit(stretch, natural, bx);
    if dest.w <= 0.0 || dest.h <= 0.0 {
        return;
    }
    if whole {
        // Drawn whole or not at all — like text lines and item icons.
        let inside = clip.is_none_or(|c| {
            let c = c.outset(crate::drawing::render::CLIP_SLACK);
            dest.x >= c.x
                && dest.y >= c.y
                && dest.right() <= c.right()
                && dest.bottom() <= c.bottom()
        });
        if inside {
            d2d.image_region(&image.source, dest, None, image.tint);
        }
        return;
    }
    // The source region shown, in image pixels.
    let region = match image.region {
        Some(r) if r.w > 0.0 && r.h > 0.0 => r,
        _ => Rect::new(0.0, 0.0, natural.x, natural.y),
    };
    let mut src = Rect::new(
        region.x + shown.x * region.w,
        region.y + shown.y * region.h,
        shown.w * region.w,
        shown.h * region.h,
    );
    let mut dest = dest;
    if let Some(c) = clip {
        let Some(cut) = dest.intersect(&c) else {
            return;
        };
        // Texture pixels per GUI pixel.
        let (sx, sy) = (src.w / dest.w, src.h / dest.h);
        src = Rect::new(
            src.x + (cut.x - dest.x) * sx,
            src.y + (cut.y - dest.y) * sy,
            cut.w * sx,
            cut.h * sy,
        );
        dest = cut;
    }
    d2d.image_region(&image.source, dest, Some(src), image.tint);
}
