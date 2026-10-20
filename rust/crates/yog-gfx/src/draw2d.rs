//! 2D drawing helpers — thin wrappers over the `draw2d_*` ABI functions.
//!
//! These use Minecraft's own rendering infrastructure (text renderer, texture
//! manager) and are therefore only valid during `on_hud_render`.

use yog_abi::YogStr;
use yog_graphics::{Color, Rect, Vec2};

use crate::GfxContext;

/// 2D drawing helpers, valid only within `on_hud_render`.
///
/// Obtain via [`GfxContext::draw2d`].
/// How text is drawn: its font (empty: the default), times its size (1 =
/// the font's own), its colour, whether it has a shadow.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TextStyle<'a> {
    pub font: &'a str,
    pub scale: f32,
    pub color: Color,
    pub shadow: bool,
}

pub struct Draw2D<'ctx>(&'ctx GfxContext);

impl<'ctx> Draw2D<'ctx> {
    pub(crate) fn new(ctx: &'ctx GfxContext) -> Self {
        Self(ctx)
    }

    /// Draws `text` at GUI position `at`, a drop shadow under it if `shadow`.
    pub fn text(&self, text: &str, at: Vec2, color: Color, shadow: bool) {
        let a = self.0.api();
        unsafe { (a.draw2d_text)(YogStr::from_str(text), at.x, at.y, color.0, shadow) }
    }

    /// [`Self::text`] in the font `font` (empty: the default one).
    pub fn text_font(&self, text: &str, font: &str, at: Vec2, color: Color, shadow: bool) {
        let a = self.0.api();
        unsafe {
            (a.draw2d_text_font)(
                YogStr::from_str(text),
                YogStr::from_str(font),
                at.x,
                at.y,
                color.0,
                shadow,
            )
        }
    }

    /// Width in GUI pixels of `text` in `font` (empty = default), measured by
    /// the host's text renderer.
    pub fn text_width(&self, text: &str, font: &str) -> f32 {
        let a = self.0.api();
        unsafe { (a.text_width)(YogStr::from_str(text), YogStr::from_str(font)) }
    }

    /// Fills `r` with a flat color.
    pub fn rect(&self, r: Rect, color: Color) {
        let a = self.0.api();
        unsafe { (a.draw2d_rect)(r.x, r.y, r.right(), r.bottom(), color.0) }
    }

    /// Fills `r` with a vertical gradient, `top` to `bottom`.
    pub fn gradient(&self, r: Rect, top: Color, bottom: Color) {
        let a = self.0.api();
        unsafe { (a.draw2d_gradient)(r.x, r.y, r.right(), r.bottom(), top.0, bottom.0) }
    }

    /// Renders an item stack (3D block models included) via Minecraft's item
    /// renderer — like inventory slots or Patchouli's item icons: item `id`
    /// (`minecraft:crafting_table`) at `at`, `size` GUI pixels big (16 = an
    /// inventory icon).
    pub fn item(&self, id: &str, at: Vec2, size: f32) {
        let a = self.0.api();
        unsafe { (a.draw2d_item)(YogStr::from_str(id), at.x, at.y, size) }
    }

    /// Draws the image `source` — a Yog image source such as `"item:apple"`,
    /// `"block:stone"`, `"sprite:button"` or `"mymod:gui/logo"` (see
    /// [`crate::image`]) — scaled into `r`. The host resolves it for its
    /// Minecraft version.
    pub fn image(&self, source: &str, r: Rect) {
        self.image_region(source, r, None, Color::WHITE);
    }

    /// [`Self::image`] drawing only `region` of it (in the image's own
    /// pixels; `None` = all of it) multiplied by `tint` (white = none).
    /// Items and sprites are always drawn whole, and items ignore the tint.
    pub fn image_region(&self, source: &str, r: Rect, region: Option<Rect>, tint: Color) {
        let a = self.0.api();
        let src = region.unwrap_or_default();
        unsafe {
            (a.draw2d_image)(
                YogStr::from_str(source),
                r.x,
                r.y,
                r.w,
                r.h,
                src.x,
                src.y,
                src.w,
                src.h,
                tint.0,
            )
        }
    }

    /// Draws the SVG document `data` scaled into `r` (sharp at any GUI
    /// scale: the host rasterizes it at the screen's resolution). An `.svg`
    /// file is an image source like any other — `mymod:gui/logo.svg` — for
    /// [`Self::image`].
    pub fn svg(&self, data: &str, r: Rect) {
        let a = self.0.api();
        unsafe { (a.draw2d_svg)(YogStr::from_str(data), r.x, r.y, r.w, r.h) }
    }

    /// `text` at `at` in `style`.
    pub fn text_scaled(&self, text: &str, at: Vec2, style: &TextStyle<'_>) {
        let a = self.0.api();
        unsafe { (a.draw2d_text_scaled)(YogStr::from_str(text), YogStr::from_str(style.font), at.x, at.y, style.scale, style.color.0, style.shadow) }
    }

    /// Draws blocks in 3D, as big as fits in `r` — a multiblock structure,
    /// a block on display: each is `((x, y, z), state)` with a full block
    /// state (`minecraft:furnace[facing=north]`), turned `yaw` degrees about
    /// the vertical and tilted `pitch` degrees toward the viewer (30 looks
    /// down on it like Patchouli does).
    pub fn blocks(&self, blocks: &[((i32, i32, i32), &str)], r: Rect, yaw: f32, pitch: f32) {
        let mut list = String::new();
        for ((bx, by, bz), state) in blocks {
            use std::fmt::Write as _;
            let _ = writeln!(list, "{bx} {by} {bz} {state}");
        }
        let a = self.0.api();
        unsafe { (a.draw2d_blocks)(YogStr::from_str(&list), r.x, r.y, r.w, r.h, yaw, pitch) }
    }

    /// Draws a living entity of type `entity` (`minecraft:zombie`,
    /// `mymod:wisp`) standing in `r`, as big as fits, turned `yaw` degrees
    /// (0 = facing the viewer) and tilted `pitch` degrees.
    pub fn entity(&self, entity: &str, r: Rect, yaw: f32, pitch: f32) {
        let a = self.0.api();
        unsafe { (a.draw2d_entity)(YogStr::from_str(entity), r.x, r.y, r.w, r.h, yaw, pitch) }
    }

    /// Natural size in GUI pixels of the image `source` (`None` if the host
    /// can't resolve it).
    pub fn image_size(&self, source: &str) -> Option<Vec2> {
        image_size_with(self.0.image_size_fn(), source)
    }
}

/// Calls an [`ImageSizeFn`](crate::ImageSizeFn).
pub fn image_size_with(f: crate::ImageSizeFn, source: &str) -> Option<Vec2> {
    let (mut w, mut h) = (0.0f32, 0.0f32);
    unsafe { f(YogStr::from_str(source), &mut w, &mut h) }.then_some(Vec2::new(w, h))
}
