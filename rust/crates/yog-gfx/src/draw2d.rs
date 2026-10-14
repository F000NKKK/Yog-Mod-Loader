//! 2D drawing helpers — thin wrappers over the `draw2d_*` ABI functions.
//!
//! These use Minecraft's own rendering infrastructure (text renderer, texture
//! manager) and are therefore only valid during `on_hud_render`.

use yog_abi::YogStr;

use crate::GfxContext;

/// 2D drawing helpers, valid only within `on_hud_render`.
///
/// Obtain via [`GfxContext::draw2d`].
pub struct Draw2D<'ctx>(&'ctx GfxContext);

impl<'ctx> Draw2D<'ctx> {
    pub(crate) fn new(ctx: &'ctx GfxContext) -> Self {
        Self(ctx)
    }

    /// Draw a text string at GUI position `(x, y)`.
    /// `color` is `0xAARRGGBB`. `shadow` adds a drop-shadow.
    pub fn text(&self, text: &str, x: f32, y: f32, color: u32, shadow: bool) {
        let a = self.0.api();
        unsafe { (a.draw2d_text)(YogStr::from_str(text), x, y, color, shadow) }
    }

    /// [`Self::text`] in the font `font` (`"minecraft:uniform"`, a
    /// resource-pack font id, ...; empty or `"minecraft:default"` = the
    /// default font).
    pub fn text_font(&self, text: &str, font: &str, x: f32, y: f32, color: u32, shadow: bool) {
        let a = self.0.api();
        unsafe {
            (a.draw2d_text_font)(
                YogStr::from_str(text),
                YogStr::from_str(font),
                x,
                y,
                color,
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

    /// Fill a rectangle with a flat color (`0xAARRGGBB`).
    pub fn rect(&self, x1: f32, y1: f32, x2: f32, y2: f32, color: u32) {
        let a = self.0.api();
        unsafe { (a.draw2d_rect)(x1, y1, x2, y2, color) }
    }

    /// Fill a rectangle with a vertical gradient (top → bottom, `0xAARRGGBB`).
    pub fn gradient(&self, x1: f32, y1: f32, x2: f32, y2: f32, top: u32, bottom: u32) {
        let a = self.0.api();
        unsafe { (a.draw2d_gradient)(x1, y1, x2, y2, top, bottom) }
    }

    /// Render an item stack (3D block models included) via Minecraft's item
    /// renderer — like inventory slots or Patchouli's item icons.
    ///
    /// - `id`: item registry id, e.g. `"minecraft:crafting_table"`
    /// - `(x, y)`: screen position in GUI pixels
    /// - `size`: on-screen size in GUI pixels (16 = inventory icon size)
    pub fn item(&self, id: &str, x: f32, y: f32, size: f32) {
        let a = self.0.api();
        unsafe { (a.draw2d_item)(YogStr::from_str(id), x, y, size) }
    }

    /// Draw the image `source` — a Yog image source such as `"item:apple"`,
    /// `"block:stone"`, `"sprite:button"` or `"mymod:gui/logo"` (see
    /// [`crate::image`]) — scaled into `(x, y, w, h)`. The host resolves it
    /// for its Minecraft version.
    pub fn image(&self, source: &str, x: f32, y: f32, w: f32, h: f32) {
        self.image_region(source, x, y, w, h, [0.0; 4], 0xFFFF_FFFF);
    }

    /// [`Self::image`] drawing only `region` (`[u, v, w, h]` in the image's
    /// own pixels; all zero = the whole image) multiplied by `tint`
    /// (`0xAARRGGBB`, `0xFFFFFFFF` = none). Items and sprites are always
    /// drawn whole, and items ignore the tint.
    #[allow(clippy::too_many_arguments)]
    pub fn image_region(
        &self,
        source: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        region: [f32; 4],
        tint: u32,
    ) {
        let a = self.0.api();
        let [u, v, uw, vh] = region;
        unsafe { (a.draw2d_image)(YogStr::from_str(source), x, y, w, h, u, v, uw, vh, tint) }
    }

    /// Draws the SVG document `data` scaled into `(x, y, w, h)` (sharp at
    /// any GUI scale: the host rasterizes it at the screen's resolution).
    /// An `.svg` file is an image source like any other —
    /// `mymod:gui/logo.svg` — for [`Self::image`].
    pub fn svg(&self, data: &str, x: f32, y: f32, w: f32, h: f32) {
        let a = self.0.api();
        unsafe { (a.draw2d_svg)(YogStr::from_str(data), x, y, w, h) }
    }

    /// [`Self::text_font`] `scale` times its size (1 = the font's own).
    #[allow(clippy::too_many_arguments)]
    pub fn text_scaled(&self, text: &str, font: &str, x: f32, y: f32, scale: f32, color: u32, shadow: bool) {
        let a = self.0.api();
        unsafe { (a.draw2d_text_scaled)(YogStr::from_str(text), YogStr::from_str(font), x, y, scale, color, shadow) }
    }

    /// Draws blocks in 3D, as big as fits in `(x, y, w, h)` — a multiblock
    /// structure, a block on display: each is `((x, y, z), state)` with a
    /// full block state (`minecraft:furnace[facing=north]`), turned `yaw`
    /// degrees about the vertical and tilted `pitch` degrees toward the
    /// viewer (30 looks down on it like Patchouli does).
    #[allow(clippy::too_many_arguments)]
    pub fn blocks(&self, blocks: &[((i32, i32, i32), &str)], x: f32, y: f32, w: f32, h: f32, yaw: f32, pitch: f32) {
        let mut list = String::new();
        for ((bx, by, bz), state) in blocks {
            use std::fmt::Write as _;
            let _ = writeln!(list, "{bx} {by} {bz} {state}");
        }
        let a = self.0.api();
        unsafe { (a.draw2d_blocks)(YogStr::from_str(&list), x, y, w, h, yaw, pitch) }
    }

    /// Draws a living entity of type `entity` (`minecraft:zombie`,
    /// `mymod:wisp`) standing in `(x, y, w, h)`, as big as fits, turned
    /// `yaw` degrees (0 = facing the viewer) and tilted `pitch` degrees.
    #[allow(clippy::too_many_arguments)]
    pub fn entity(&self, entity: &str, x: f32, y: f32, w: f32, h: f32, yaw: f32, pitch: f32) {
        let a = self.0.api();
        unsafe { (a.draw2d_entity)(YogStr::from_str(entity), x, y, w, h, yaw, pitch) }
    }

    /// Natural size in GUI pixels of the image `source` (`None` if the host
    /// can't resolve it).
    pub fn image_size(&self, source: &str) -> Option<(f32, f32)> {
        image_size_with(self.0.image_size_fn(), source)
    }
}

/// Calls an [`ImageSizeFn`](crate::ImageSizeFn).
pub fn image_size_with(f: crate::ImageSizeFn, source: &str) -> Option<(f32, f32)> {
    let (mut w, mut h) = (0.0f32, 0.0f32);
    unsafe { f(YogStr::from_str(source), &mut w, &mut h) }.then_some((w, h))
}
