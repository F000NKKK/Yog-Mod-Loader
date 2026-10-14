//! yog-ui — retained-mode UI framework for Yog mods.
//!
//! Flexbox-inspired layout engine and a WPF-style [`grid`](widget::grid)
//! (rows/columns: pixels, `Auto`, `*`) + GPU rendering via [`yog-gfx`].
//! Use for custom inventories, guide books, tooltips, HUD overlays;
//! [`surface`]s draw whatever the widgets don't (any `yog-gfx` call).
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

pub mod component;
pub mod event;
pub mod image;
pub mod layout;
mod render;
pub mod scroll;
#[cfg(test)]
mod state_tests;
pub mod surface;
pub mod theme;
pub mod text;
pub mod textbox;
pub mod widget;

pub use event::UiEvent;
pub use layout::{Align, FlexDir, GridLines, LayoutNode, Rect, hit_path, node_at, set_focus};
pub use scroll::ScrollState;
pub use surface::{Pointer, Surface};
pub use textbox::{TextBoxEvent, TextBoxState};
pub use widget::{
    Action, Cell, Dock, FocusStyle, GridLength, GridSpec, HAlign, StateFlags, StateStyles, Stretch,
    StylePatch, Track, Units, VAlign, Widget,
};

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
        Self {
            id: id.into(),
            root,
            layout_root: LayoutNode::default(),
            needs_layout: true,
        }
    }

    /// Recalculate layout. Call after changing the tree or on window resize.
    pub fn layout(&mut self, screen_w: f32, screen_h: f32) {
        self.layout_root = layout::compute(&self.root, screen_w, screen_h);
        self.needs_layout = false;
    }

    /// Render the UI tree via `yog-gfx` draw2d.
    /// Must be called from `on_hud_render`.
    pub fn render(&self, ctx: &GfxContext) {
        text::use_host_metrics(ctx);
        image::use_host_images(ctx);
        let d2d = ctx.draw2d();
        render::render_node(ctx, &d2d, &self.root, &self.layout_root);
    }

    /// The tooltip of the innermost widget at `(mx, my)` that has one.
    /// Call after [`Self::layout`].
    pub fn tooltip_at(&self, mx: f32, my: f32) -> Option<&str> {
        let path = layout::deepest_path(&self.layout_root, mx, my)?;
        let mut w = &self.root;
        let mut found = w.tooltip.as_deref();
        for &i in &path {
            let Some(c) = w.children.get(i as usize) else { break };
            w = c;
            found = w.tooltip.as_deref().or(found);
        }
        found
    }

    /// Draws `text` as a vanilla tooltip next to the cursor at `(mx, my)`,
    /// kept inside the screen. Call after [`Self::render`] so it's on top.
    pub fn render_tooltip(ctx: &GfxContext, text: &str, mx: f32, my: f32) {
        text::use_host_metrics(ctx);
        let (sw, sh) = ctx.screen_size();
        render::tooltip(&ctx.draw2d(), text, mx, my, sw as f32, sh as f32);
    }

    /// Find the deepest clickable widget at `(mx, my)` in screen coordinates.
    pub fn hit_test(&self, mx: f32, my: f32) -> Option<&LayoutNode> {
        layout::hit_test(&self.layout_root, mx, my)
    }

    /// Resolved rect of the (first) node tagged `id` — see [`layout::find_by_id`].
    /// Call after [`Self::layout`].
    pub fn find_rect(&self, id: &str) -> Option<Rect> {
        layout::find_by_id(&self.layout_root, id)
    }

    /// Laid-out node of the (first) widget tagged `id` — rect plus scroll
    /// state, see [`layout::find_node`]. Call after [`Self::layout`].
    pub fn find_node(&self, id: &str) -> Option<&LayoutNode> {
        layout::find_node(&self.layout_root, id)
    }
}
