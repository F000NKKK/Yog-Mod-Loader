//! `<Image>` in the designer: Yog image sources (`item:apple`,
//! `sprite:button`, `mod:gui/logo`, `mymod:gui/logo` — see
//! `yog_gfx::image`) resolved against the local game files and the mod's
//! `assets/`, the way the Yog runtime resolves them in game: yog-ui's
//! `Image` widget lays out and fits the picture, then calls `draw2d_image`
//! / `image_size`, which the recording table (`gfx`) answers from here.
//! Also the Properties window's `source` picker ([`candidates`],
//! [`preview`]).

use crate::content::refs::RefKind;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use yog_sync::Arc;

use yog_gfx::image::{
    ImageSource, LegacyRegion, SPRITES, Sprite, SpriteSource, Stretch, atlas_texture, fit,
    sprite as find_sprite,
};
use yog_gfx::{Color, Rect, Vec2};
use yog_ui::widget::{self, Widget};

use super::assets::{Image, Resources};
use super::markup::{YuiNode, color};
use super::raster::Canvas;

struct Doc {
    res: Resources,
    mod_id: Option<String>,
}

thread_local! {
    static DOC: RefCell<Option<Doc>> = const { RefCell::new(None) };
}

/// `[mod] id` from `<root>/yog.toml`.
/// The `.yui` being laid out and drawn: its mod's resources (images, fonts)
/// and id (for `mod:` sources). Set before every layout.
pub(crate) fn set_document(mod_root: Option<&Path>) {
    let doc = Doc {
        res: Resources::for_mod(mod_root),
        mod_id: mod_root.and_then(crate::design::project::mod_id),
    };
    DOC.with(|d| *d.borrow_mut() = Some(doc));
}

/// The current document's resources (vanilla only without one).
pub(crate) fn resources() -> Resources {
    DOC.with(|d| d.borrow().as_ref().map(|d| d.res.clone()))
        .unwrap_or_else(|| Resources::for_mod(None))
}

fn with_doc<R>(f: impl FnOnce(Option<&Doc>) -> R) -> R {
    DOC.with(|d| f(d.borrow().as_ref()))
}

/// `mod:<path>` → `<mod id>:<path>` (what `yog build` writes into the
/// generated code); other sources unchanged.
pub(crate) fn resolve_mod(source: &str) -> String {
    let source = source.trim();
    match source.strip_prefix("mod:") {
        Some(path) => match with_doc(|d| d.and_then(|d| d.mod_id.clone())) {
            Some(id) => format!("{id}:{path}"),
            None => source.to_string(),
        },
        None => source.to_string(),
    }
}

/// `stretch` attribute → [`Stretch`] (case-insensitive, like `yog build`).
pub(crate) fn stretch(v: &str) -> Stretch {
    Stretch::parse(v).unwrap_or_default()
}

/// `uv="u v w h"`.
fn uv(v: &str) -> Option<Rect> {
    let p: Vec<f32> = v
        .split([' ', ','])
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().ok())
        .collect::<Option<_>>()?;
    let [x, y, w, h] = <[f32; 4]>::try_from(p).ok()?;
    (w > 0.0 && h > 0.0).then(|| Rect::new(x, y, w, h))
}

/// The widget for an `<Image>` — what the generated code builds: `source`,
/// `stretch`, `uv`, `tint`.
pub(crate) fn widget(node: &YuiNode) -> Widget {
    let a = |k: &str| node.attr(k);
    let source = a(yog_ids::yui_attr::SOURCE).unwrap_or_default();
    let mut w = widget::image(resolve_mod(source));
    if let Some(s) = a(yog_ids::yui_attr::STRETCH) {
        w = w.stretch(stretch(s));
    }
    if let Some(r) = a(yog_ids::yui_attr::UV).and_then(uv) {
        w = w.region(r);
    }
    if let Some(t) = a(yog_ids::yui_attr::TINT).and_then(color) {
        w = w.tint(Color(t));
    }
    w
}

fn texture(id: &str) -> Option<Arc<Image>> {
    if let Some(sheet) = book_sheet(id) {
        return Some(sheet);
    }
    with_doc(|d| d.and_then(|d| d.res.texture(id)))
}

/// The book's sprite sheets — shipped in the loader's jars, embedded in
/// yog-book (see `yog_book::BOOK_SHEET`).
fn book_sheet(id: &str) -> Option<Arc<Image>> {
    static SHEETS: yog_sync::OnceLock<[Option<Arc<Image>>; 2]> = yog_sync::OnceLock::new();
    let [book, crafting] = SHEETS.get_or_init(|| {
        [
            super::assets::decode_png(yog_book::BOOK_PNG).map(Arc::new),
            super::assets::decode_png(yog_book::CRAFTING_PNG).map(Arc::new),
        ]
    });
    let texture = |sheet: &str| match ImageSource::parse(sheet) {
        Some(ImageSource::Texture(t)) => t,
        _ => String::new(),
    };
    match id {
        _ if id == texture(yog_book::BOOK_SHEET) => book.clone(),
        _ if id == texture(yog_book::CRAFTING_SHEET) => crafting.clone(),
        _ => None,
    }
}

/// A GUI-atlas sprite's picture (`minecraft:widget/button` → its png).
fn atlas_image(id: &str) -> Option<Arc<Image>> {
    let (ns, path) = yog_ids::split(id);
    texture(&atlas_texture(ns, path))
}

/// The source of `sp` the game files (or Yog) have — the first, as the host picks it.
fn resolve(sp: &Sprite) -> Option<SpriteSource<'_>> {
    sp.sources().into_iter().find(|s| match s {
        SpriteSource::Atlas(id) => atlas_image(id).is_some(),
        SpriteSource::Region(r) => texture(&r.texture).is_some(),
    })
}

/// `id` as the game files the designer reads call it: itself, or what
/// that version renamed it to (`minecraft:chain` is `iron_chain` from 1.21.9).
fn in_game_files(kind: RefKind, id: String) -> String {
    let version = with_doc(|d| d.and_then(|d| d.res.vanilla.as_ref().map(|v| v.version.clone())));
    version
        .and_then(|v| crate::content::refs::equivalent(kind, &id, &v))
        .unwrap_or(id)
}

/// The texture an item/block icon is drawn with (the designer doesn't run
/// the 3D item renderer: block items show a face of the block).
fn item_texture(id: &str, block: bool) -> Option<Arc<Image>> {
    // Its item model's layers, when it has some.
    if !block
        && let Some(super::item_models::Look::Flat(img)) =
            with_doc(|d| d.and_then(|d| super::item_models::look(&d.res, id)))
    {
        return Some(img);
    }
    let (ns, name) = yog_ids::split(id);
    let item = format!("{ns}:textures/item/{name}.png");
    let blocks =
        ["", "_front", "_side", "_top"].map(|s| format!("{ns}:textures/block/{name}{s}.png"));
    let mut order: Vec<&String> = blocks.iter().collect();
    if block {
        order.push(&item);
    } else {
        order.insert(0, &item);
    }
    order
        .into_iter()
        .find_map(|t| texture(t))
        .or_else(|| content_icon(id))
}

/// A mod item's icon from its `content/` file (cached per id).
fn content_icon(id: &str) -> Option<Arc<Image>> {
    type Cache = yog_sync::Mutex<std::collections::HashMap<(PathBuf, String), Option<Arc<Image>>>>;
    static CACHE: yog_sync::OnceLock<Cache> = yog_sync::OnceLock::new();
    let root = with_doc(|d| d.and_then(|d| d.res.mod_root.clone()))?;
    let key = (root.clone(), id.to_string());
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().get(&key) {
        return hit.clone();
    }
    let img = crate::design::content::content_icon(&root, id)
        .and_then(|b| super::assets::decode_png(&b))
        .map(Arc::new);
    cache.lock().insert(key, img.clone());
    img
}

/// A block item as the game shows it in a slot (the block in 3D, turned
/// 225°, tilted 30°), or a head (its mob's, a little smaller). `false`:
/// a flat item. As in the game, the item's model decides: one drawn from
/// texture layers is flat even when there's a block of the same id (a
/// brewing stand, a door, a cauldron).
fn item_3d(c: &mut Canvas, id: &str, dest: Rect) -> bool {
    match super::item_models::look(c.resources(), id) {
        Some(super::item_models::Look::Flat(_)) => false,
        Some(super::item_models::Look::Head(faces)) => {
            let m = dest.w.min(dest.h) * HEAD_MARGIN;
            super::blocks3d::draw_cube(c, faces, dest.inset([m; 4]), SLOT_YAW, SLOT_PITCH);
            true
        }
        None if super::blocks3d::is_block(c.resources(), id) => {
            super::blocks3d::draw(c, &format!("0 0 0 {id}"), dest, SLOT_YAW, SLOT_PITCH);
            true
        }
        None => false,
    }
}

/// How a block is turned and tilted in a slot (the game's GUI display; the book's).
use yog_book::renderer::{BLOCK_PITCH as SLOT_PITCH, SLOT_YAW};
/// How much smaller than the slot a head is, on each side.
const HEAD_MARGIN: f32 = 0.1;

/// Natural size of `source` in GUI pixels (`image_size`); `None` when it
/// can't be resolved.
pub(crate) fn source_size(source: &str) -> Option<Vec2> {
    match ImageSource::parse(source)? {
        ImageSource::Item(_) | ImageSource::Block(_) => {
            Some(Vec2::new(yog_ui::ICON_SIZE, yog_ui::ICON_SIZE))
        }
        ImageSource::Sprite(name) => find_sprite(&name).map(|s| match resolve(s) {
            Some(SpriteSource::Region(r)) => Vec2::new(r.region.w, r.region.h),
            _ => s.size,
        }),
        ImageSource::Texture(id) if id.ends_with(".svg") => {
            svg_document(&id).and_then(|d| yog_media::svg::size(&d))
        }
        ImageSource::Texture(id) => {
            texture(&id).map(|i| Vec2::new(i.width as f32, i.height as f32))
        }
        ImageSource::ModPath(_) => None,
    }
}

/// `img`'s pixels `src` into GUI `dest`, colors times `t`.
fn blit_px(c: &mut Canvas, img: &Image, dest: Rect, src: Rect, t: Color) {
    if dest.w <= 0.0 || dest.h <= 0.0 || src.w <= 0.0 || src.h <= 0.0 {
        return;
    }
    let (iw, ih) = (img.width as f32, img.height as f32);
    c.blit(
        img,
        dest,
        Rect::new(src.x / iw, src.y / ih, src.w / iw, src.h / ih),
        t,
    );
}

/// All of `img`, in its pixels.
fn whole(img: &Image) -> Rect {
    Rect::new(0.0, 0.0, img.width as f32, img.height as f32)
}

/// The inventory slot (drawn by Yog on every version).
fn slot(c: &mut Canvas, dest: Rect) {
    for (r, color) in yog_ui::inv_slot_bevel(dest) {
        c.fill(r, color, color);
    }
}

/// Sprite `sp` into `dest`: nine-sliced by its border, or — `part` a region
/// of it in its own pixels (`w`, `h` > 0) — that part, scaled.
fn sprite(c: &mut Canvas, sp: &Sprite, dest: Rect, part: Rect, t: Color) {
    let all = part.w <= 0.0 || part.h <= 0.0;
    // `src` (a region of the image whose sprite is `natural` pixels wide) drawn whole or in part.
    let mut put = |img: &Image, src: Rect, natural: f32, border: f32| {
        if all {
            for (to, from) in yog_gfx::image::nine_slice(dest, src, border) {
                blit_px(c, img, to, from, t);
            }
        } else {
            let k = src.w / natural;
            let from = Rect::new(
                src.x + part.x * k,
                src.y + part.y * k,
                part.w * k,
                part.h * k,
            );
            blit_px(c, img, dest, from, t);
        }
    };
    if sp.is_procedural() {
        slot(c, dest);
        return;
    }
    let source = resolve(sp);
    if let Some(SpriteSource::Atlas(id)) = &source
        && let Some(img) = atlas_image(id)
    {
        // Animated sprites stack their frames vertically: the first one.
        let size = sp.size;
        let frame_h = if img.height > img.width && size.y <= size.x {
            img.width as f32 * size.y / size.x
        } else {
            img.height as f32
        };
        put(
            &img,
            Rect::new(0.0, 0.0, img.width as f32, frame_h),
            size.x,
            sp.border,
        );
        return;
    }
    if let Some(SpriteSource::Region(LegacyRegion {
        texture: tex,
        region,
        texture_size,
    })) = source
        && let Some(img) = texture(tex)
    {
        // Resource packs may ship the sheet at a higher resolution.
        let k = img.width as f32 / texture_size.x;
        put(&img, region.scale(k), region.w, sp.border * k);
        return;
    }
    missing(c, dest);
}

fn missing(c: &mut Canvas, dest: Rect) {
    let icon = Rect::new(0.0, 0.0, yog_ui::ICON_SIZE, yog_ui::ICON_SIZE);
    blit_px(c, &super::assets::missing_image(), dest, icon, Color::WHITE);
}

/// Physical pixels an SVG is rasterized at per GUI pixel.
const SVG_OVERSAMPLE: f32 = 4.0;

/// An SVG document as an image, `w`×`h` — as the runtime rasterizes it in game.
fn svg_image(data: &str, w: u32, h: u32) -> Option<Image> {
    Some(Image {
        width: w,
        height: h,
        rgba: yog_media::svg::rasterize(data, w, h)?,
    })
}

/// The document of an `.svg` texture id (`ns:textures/x.svg`) of the mod.
fn svg_document(id: &str) -> Option<String> {
    let (ns, path) = yog_ids::split(id);
    let bytes = with_doc(|d| d.and_then(|d| d.res.read(ns, path)))?;
    String::from_utf8(bytes).ok()
}

/// `draw2d_svg`: `data` into `dest`.
pub(crate) fn draw_svg(c: &mut Canvas, data: &str, dest: Rect, t: Color) {
    if dest.w <= 0.0 || dest.h <= 0.0 {
        return;
    }
    let side = |v: f32| ((v * SVG_OVERSAMPLE).ceil() as u32).clamp(1, 2048);
    match svg_image(data, side(dest.w), side(dest.h)) {
        Some(img) => blit_px(c, &img, dest, whole(&img), t),
        None => missing(c, dest),
    }
}

/// An entity's spawn egg is drawn at most this big (two icons).
const ENTITY_EGG_MAX: f32 = 2.0 * yog_ui::ICON_SIZE;

/// `draw2d_entity`: the designer has no entity models — the entity's spawn
/// egg, in the middle of its box.
pub(crate) fn draw_entity(c: &mut Canvas, entity: &str, area: Rect) {
    let (ns, path) = yog_ids::split(entity);
    let side = area.w.min(area.h).min(ENTITY_EGG_MAX);
    let dest = Rect::new(
        area.x + (area.w - side) / 2.0,
        area.y + (area.h - side) / 2.0,
        side,
        side,
    );
    let egg = yog_ids::join(ns, &format!("{path}{}", yog_ids::res::SPAWN_EGG_SUFFIX));
    draw(
        c,
        &yog_ids::join(yog_ids::image_scheme::ITEM, &egg),
        dest,
        Rect::default(),
        Color::WHITE,
    );
}

/// `draw2d_image`: `source` into `dest` (`x y w h`, already fitted by
/// yog-ui); `region` `u v w h` in image pixels (`w <= 0`: whole image).
pub(crate) fn draw(c: &mut Canvas, source: &str, dest: Rect, region: Rect, t: Color) {
    if dest.w <= 0.0 || dest.h <= 0.0 {
        return;
    }
    if let Some(ImageSource::Texture(id)) = ImageSource::parse(source)
        && id.ends_with(".svg")
    {
        match svg_document(&id) {
            Some(doc) => draw_svg(c, &doc, dest, t),
            None => missing(c, dest),
        }
        return;
    }
    let first_frame =
        |img: &Image| Rect::new(0.0, 0.0, img.width as f32, img.width.min(img.height) as f32);
    // Content the game renamed is shown by its name in the game files at hand.
    let source = match ImageSource::parse(source) {
        Some(ImageSource::Item(id)) => Some(ImageSource::Item(in_game_files(RefKind::Item, id))),
        Some(ImageSource::Block(id)) => Some(ImageSource::Block(in_game_files(RefKind::Block, id))),
        other => other,
    };
    match source {
        Some(ImageSource::Item(id)) if item_3d(c, &id, dest) => {}
        Some(ImageSource::Item(id)) => match item_texture(&id, false) {
            Some(img) => blit_px(c, &img, dest, first_frame(&img), Color::WHITE),
            None => missing(c, dest),
        },
        Some(ImageSource::Block(id)) => match item_texture(&id, true) {
            Some(img) => blit_px(c, &img, dest, first_frame(&img), Color::WHITE),
            None => missing(c, dest),
        },
        Some(ImageSource::Sprite(name)) => match find_sprite(&name) {
            Some(sp) => sprite(c, sp, dest, region, t),
            None => match name
                .split_once(':')
                .and_then(|(ns, p)| texture(&atlas_texture(ns, p)))
            {
                Some(img) => blit_px(c, &img, dest, whole(&img), t),
                None => missing(c, dest),
            },
        },
        Some(ImageSource::Texture(id)) => match texture(&id) {
            Some(img) => {
                let src = if region.w > 0.0 && region.h > 0.0 {
                    region
                } else {
                    whole(&img)
                };
                blit_px(c, &img, dest, src, t);
            }
            None => missing(c, dest),
        },
        Some(ImageSource::ModPath(_)) | None => missing(c, dest),
    }
}

/// An alias the Properties picker offers.
#[derive(Debug, Clone, yog_data::Schema)]
pub(crate) struct Candidate {
    /// The source to write (`item:apple`, `sprite:button`, `mod:gui/logo`).
    pub(crate) source: String,
    /// `item`, `block`, `sprite` or `mod`.
    pub(crate) group: String,
    pub(crate) detail: String,
}

/// Everything the `source` picker offers for the `.yui` at `path`: the
/// mod's own textures, the sprite table, then the game's items and blocks.
pub(crate) fn candidates(path: &Path) -> Vec<Candidate> {
    let root = super::mod_root(path);
    let mut out = Vec::new();
    if let Some(root) = &root {
        out.extend(crate::media::pictures(root).into_iter().map(|p| Candidate {
            source: ImageSource::ModPath(p.path()).to_string(),
            group: "mod".into(),
            detail: format!("@{}.{}", p.set, p.name),
        }));
    }
    out.extend(SPRITES.iter().map(|s| Candidate {
        source: format!("sprite:{}", s.name),
        group: "sprite".into(),
        detail: format!("{} ({}×{})", s.description, s.size.x, s.size.y),
    }));
    let res = Resources::for_mod(root.as_deref());
    if let Some(v) = &res.vanilla {
        let (items, blocks) = v.texture_names();
        out.extend(items.into_iter().map(|n| Candidate {
            source: yog_ids::join(yog_ids::image_scheme::ITEM, &n),
            group: yog_ids::image_scheme::ITEM.into(),
            detail: String::new(),
        }));
        out.extend(blocks.into_iter().map(|n| Candidate {
            source: yog_ids::join(yog_ids::image_scheme::BLOCK, &n),
            group: yog_ids::image_scheme::BLOCK.into(),
            detail: String::new(),
        }));
    }
    out
}

/// A preview of an image — the one way the IDE draws any image reference:
/// its PNG data URL (`size` px, nearest-neighbour; `None` if it can't be
/// drawn) and, for a raw resource path (`minecraft:textures/item/apple.png`),
/// the alias to write instead (`item:apple`) — which is also what's drawn.
#[derive(yog_data::Schema)]
pub(crate) struct Preview {
    pub(crate) url: Option<String>,
    pub(crate) alias: Option<String>,
}

pub(crate) fn preview(path: &Path, source: &str, size: u32) -> Preview {
    let alias = yog_gfx::image::suggest_alias(source);
    let url = draw_preview(path, alias.as_deref().unwrap_or(source), size);
    Preview { url, alias }
}

/// The smallest and largest square preview.
const MIN_PREVIEW: u32 = 8;
const MAX_PREVIEW: u32 = 256;
/// An item icon's side, the size of a source that doesn't say its own.
const ICON_SIDE: f32 = 16.0;

fn draw_preview(path: &Path, source: &str, size: u32) -> Option<String> {
    let root = super::mod_root(path);
    set_document(root.as_deref());
    let res = Resources::for_mod(root.as_deref());
    let source = resolve_mod(source);
    let natural = source_size(&source).unwrap_or(Vec2::new(ICON_SIDE, ICON_SIDE));
    let size = size.clamp(MIN_PREVIEW, MAX_PREVIEW);
    let (dest, _) = fit(
        Stretch::Uniform,
        natural,
        Rect::new(0.0, 0.0, size as f32, size as f32),
    );
    let mut c = Canvas::new(size, size, 1, &res);
    draw(&mut c, &source, dest, Rect::default(), Color::WHITE);
    c.png_url()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `YOG_IMAGES_PNG=out.png cargo test --lib designer::images -- --ignored`:
    /// every kind of source drawn with the local game files, for eyeballing.
    #[test]
    #[ignore]
    fn sample_png() {
        use base64::Engine as _;
        let Ok(out) = std::env::var(yog_ids::env::IMAGES_PNG) else {
            return;
        };
        let n = |uid: &str, attrs: &[(&str, &str)]| YuiNode {
            uid: uid.into(),
            kind: "Image".into(),
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            children: vec![],
        };
        let mut root = n(
            "s",
            &[("width", "200"), ("height", "120"), ("bg", "#C0101010")],
        );
        root.kind = "Screen".into();
        root.children = vec![
            // w/h larger than the texture.
            n(
                "a",
                &[
                    ("x", "4"),
                    ("y", "4"),
                    ("w", "156"),
                    ("h", "29"),
                    ("source", "minecraft:item/apple"),
                    ("bg", "#40FFFFFF"),
                ],
            ),
            n(
                "b",
                &[
                    ("x", "4"),
                    ("y", "36"),
                    ("w", "32"),
                    ("h", "32"),
                    ("source", "item:apple"),
                ],
            ),
            n(
                "c",
                &[
                    ("x", "40"),
                    ("y", "36"),
                    ("w", "32"),
                    ("h", "32"),
                    ("source", "block:crafting_table"),
                ],
            ),
            n(
                "d",
                &[
                    ("x", "76"),
                    ("y", "36"),
                    ("w", "100"),
                    ("h", "20"),
                    ("source", "sprite:button"),
                    ("stretch", "Fill"),
                ],
            ),
            n(
                "e",
                &[("x", "76"), ("y", "60"), ("source", "sprite:icon/heart")],
            ),
            n("f", &[("x", "90"), ("y", "60"), ("source", "sprite:slot")]),
            n(
                "g",
                &[
                    ("x", "4"),
                    ("y", "72"),
                    ("w", "64"),
                    ("h", "40"),
                    ("source", "item:diamond"),
                    ("stretch", "Fill"),
                    ("tint", "#FF80FF80"),
                ],
            ),
        ];
        let args = super::super::RenderArgs {
            path: "/nowhere/src/x.yui".into(),
            root,
            screen_w: 200,
            screen_h: 120,
            scale: 4,
            preview: false,
        };
        let r = super::super::render(&args).unwrap();
        let b64 = r
            .image
            .trim_start_matches(super::super::raster::PNG_URL_PREFIX);
        std::fs::write(
            out,
            base64::engine::general_purpose::STANDARD
                .decode(b64)
                .unwrap(),
        )
        .unwrap();
    }
}
