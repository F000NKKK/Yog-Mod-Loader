//! yog-ui — retained-mode UI framework for Yog mods.
//!
//! Flexbox-inspired layout engine + GPU rendering via [`yog-gfx`].
//! Use for custom inventories, guide books, tooltips, HUD overlays.
//!
//! # Quick start
//! ```ignore
//! use yog_ui::{UiRoot, widget, Align, FlexDir, Units};
//!
//! let ui = UiRoot::new("mymod:main_menu")
//!     .style(|s| s.bg(0x88332211).padding(8.0, 8.0, 8.0, 8.0))
//!     .child(
//!         widget::panel(FlexDir::Column).gap(4.0)
//!             .child(widget::label("Hello, World!").color(0xFF_DDAA00))
//!             .child(widget::button("Click me").on_click("mymod:btn_click"))
//!     );
//! ```

pub mod layout;
mod render;
pub mod slot_cache;
pub mod text;
pub mod widget;

pub use layout::{Align, FlexDir, LayoutNode, Rect, set_focus};
pub use widget::{Dock, FocusStyle, Widget};

use yog_gfx::GfxContext;

/// Top-level UI tree.  Build it, call [`layout`], then [`render`] each frame.
pub struct UiRoot {
    pub id: String,
    pub root: Widget,
    pub layout_root: LayoutNode,
    pub needs_layout: bool,
}

impl UiRoot {
    pub fn new(id: impl Into<String>, root: Widget) -> Self {
        Self { id: id.into(), root, layout_root: LayoutNode::default(), needs_layout: true }
    }

    /// Recalculate layout. Call after changing the tree or on window resize.
    pub fn layout(&mut self, screen_w: f32, screen_h: f32) {
        self.layout_root = layout::compute(&self.root, screen_w, screen_h);
        self.needs_layout = false;
    }

    /// Render the UI tree via `yog-gfx` draw2d.
    /// Must be called from `on_hud_render`.
    pub fn render(&self, ctx: &GfxContext) {
        let d2d = ctx.draw2d();
        render::render_node(&d2d, &self.root, &self.layout_root);
    }

    /// Find the deepest clickable widget at `(mx, my)` in screen coordinates.
    pub fn hit_test(&self, mx: f32, my: f32) -> Option<&LayoutNode> {
        layout::hit_test(&self.layout_root, mx, my)
    }
}
