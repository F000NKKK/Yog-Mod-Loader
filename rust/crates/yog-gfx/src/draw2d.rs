//! 2D drawing helpers — thin wrappers over the `draw2d_*` ABI functions.
//!
//! These use Minecraft's own rendering infrastructure (text renderer, texture
//! manager) and are therefore only valid during `on_hud_render`.

use yog_abi::YogStr;

use crate::{GfxContext, InvSlotData};

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

    /// Blit a region from a Minecraft texture identified by namespace + path.
    ///
    /// - `id`: e.g. `"minecraft:textures/gui/icons.png"`
    /// - `(x, y)`: screen position
    /// - `(u0, v0)`: top-left UV in texels
    /// - `(w, h)`: region size in pixels
    /// - `(tw, th)`: full texture size in pixels
    pub fn mc_texture(
        &self,
        id: &str,
        x: f32,
        y: f32,
        u0: f32,
        v0: f32,
        w: f32,
        h: f32,
        tw: f32,
        th: f32,
    ) {
        let a = self.0.api();
        unsafe { (a.draw2d_mc_tex)(YogStr::from_str(id), x, y, u0, v0, w, h, tw, th) }
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

    /// Number of slots snapshotted this frame from the currently-open
    /// inventory screen (0 if none is open). See [`GfxContext::inv_slot_count`].
    pub fn inv_slot_count(&self) -> usize {
        self.0.inv_slot_count()
    }

    /// Slot `index`'s content this frame. See [`GfxContext::inv_slot`].
    pub fn inv_slot(&self, index: usize) -> Option<InvSlotData> {
        self.0.inv_slot(index)
    }
}
