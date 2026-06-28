//! Widget types and styling.

use crate::layout::{Align, FlexDir};

/// A single widget in the UI tree.
#[derive(Debug, Clone)]
pub struct Widget {
    pub kind:     WidgetKind,
    pub id:       Option<String>,
    pub style:    Style,
    pub flex_dir: FlexDir,
    pub children: Vec<Widget>,
    pub on_click: Option<String>,
}

#[derive(Debug, Clone)]
pub enum WidgetKind {
    /// Container — arranges children according to `flex_dir`.
    Panel,
    /// Static or dynamic text label.
    Label(String),
    /// Clickable button with text.
    Button(String),
    /// Minecraft item icon.
    ItemSlot(String),
    /// Minecraft texture blit (via `draw2d_mc_tex`).
    McImage { id: String, img_w: f32, img_h: f32 },
    /// Invisible spacer.
    Spacer,
}

/// Visual and layout style for a widget.
#[derive(Debug, Clone)]
pub struct Style {
    pub w:      f32,   // explicit width; 0 = auto
    pub h:      f32,   // explicit height; 0 = auto
    pub min_w:  f32,
    pub min_h:  f32,
    pub flex:   f32,   // grow factor inside flex container
    pub gap:    f32,   // spacing between children
    pub pad:    [f32; 4],  // top, right, bottom, left
    pub margin: [f32; 4],
    pub bg:     u32,   // background colour 0xAARRGGBB; 0 = transparent
    pub color:  u32,   // text colour
    pub align:  Align,
    pub font_scale: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            w: 0.0, h: 0.0, min_w: 4.0, min_h: 4.0, flex: 0.0, gap: 2.0,
            pad: [0.0; 4], margin: [0.0; 4], bg: 0, color: 0xFF_CCCCAA,
            align: Align::Start, font_scale: 1.0,
        }
    }
}

// ── Builder API ───────────────────────────────────────────────────────────────

impl Widget {
    fn new(kind: WidgetKind) -> Self {
        Self { kind, id: None, style: Style::default(), flex_dir: FlexDir::Column,
               children: Vec::new(), on_click: None }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self { self.id = Some(id.into()); self }
    pub fn on_click(mut self, ev: impl Into<String>) -> Self { self.on_click = Some(ev.into()); self }
    pub fn child(mut self, w: Widget) -> Self { self.children.push(w); self }

    pub fn w(mut self, w: f32) -> Self { self.style.w = w; self }
    pub fn h(mut self, h: f32) -> Self { self.style.h = h; self }
    pub fn min_w(mut self, v: f32) -> Self { self.style.min_w = v; self }
    pub fn min_h(mut self, v: f32) -> Self { self.style.min_h = v; self }
    pub fn flex(mut self, v: f32) -> Self { self.style.flex = v; self }
    pub fn flex_dir(mut self, v: FlexDir) -> Self { self.flex_dir = v; self }
    pub fn gap(mut self, v: f32) -> Self { self.style.gap = v; self }
    pub fn bg(mut self, v: u32) -> Self { self.style.bg = v; self }
    pub fn color(mut self, v: u32) -> Self { self.style.color = v; self }
    pub fn align(mut self, v: Align) -> Self { self.style.align = v; self }
    pub fn font_scale(mut self, v: f32) -> Self { self.style.font_scale = v; self }
    pub fn padding(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        self.style.pad = [top, right, bottom, left]; self
    }
    pub fn margin(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        self.style.margin = [top, right, bottom, left]; self
    }
}

// ── Widget constructors ───────────────────────────────────────────────────────

pub fn panel(dir: FlexDir) -> Widget { Widget::new(WidgetKind::Panel).flex_dir(dir) }
pub fn label(text: impl Into<String>) -> Widget { Widget::new(WidgetKind::Label(text.into())) }
pub fn button(text: impl Into<String>) -> Widget { Widget::new(WidgetKind::Button(text.into())) }
pub fn item_slot(item_id: impl Into<String>) -> Widget { Widget::new(WidgetKind::ItemSlot(item_id.into())) }
pub fn mc_image(id: impl Into<String>, img_w: f32, img_h: f32) -> Widget {
    Widget::new(WidgetKind::McImage { id: id.into(), img_w, img_h })
        .w(img_w).h(img_h)
}
pub fn spacer() -> Widget { Widget::new(WidgetKind::Spacer) }
