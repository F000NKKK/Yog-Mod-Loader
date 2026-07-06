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
    pub enabled:  bool,
    pub focused:  bool,
}

#[derive(Debug, Clone)]
pub enum WidgetKind {
    /// Container — arranges children according to `flex_dir`.
    Panel(FlexDir),
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

/// WinForms-style dock — which edge(s) the widget attaches to inside its parent.
///
/// - `None`   — normal flex positioning (default)
/// - `Fill`   — stretch to fill all remaining space (both axes)
/// - `Left`   — full height, natural width, hugs left edge; other children flow right
/// - `Right`  — full height, natural width, hugs right edge
/// - `Top`    — full width, natural height, hugs top edge; other children flow down
/// - `Bottom` — full width, natural height, hugs bottom edge
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Dock { #[default] None, Fill, Left, Right, Top, Bottom }

/// How a focused widget shows its focus indicator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusStyle {
    /// 1px outline using `focus_color` (default, amber).
    Outline,
    /// Solid background fill using `focus_color`.
    Fill,
    /// No visual indicator.
    None,
}

impl Default for FocusStyle { fn default() -> Self { FocusStyle::Outline } }

/// Visual and layout style for a widget.
#[derive(Debug, Clone)]
pub struct Style {
    pub w:      f32,   // explicit width; 0 = auto
    pub h:      f32,   // explicit height; 0 = auto
    pub min_w:  f32,
    pub min_h:  f32,
    pub flex:   f32,   // grow factor inside flex container (main axis)
    pub dock:   Dock,  // WinForms-style edge attachment
    pub gap:    f32,   // spacing between children
    pub pad:    [f32; 4],  // top, right, bottom, left — space INSIDE the border
    pub margin: [f32; 4],  // top, right, bottom, left — space OUTSIDE the border
    pub bg:     u32,   // background colour 0xAARRGGBB; 0 = transparent
    pub color:  u32,   // text colour
    pub align:  Align,
    pub font_scale:  f32,
    pub text_shadow: bool, // MC drop-shadow behind text (HUD default; books disable it)
    pub focus_style: FocusStyle,
    pub focus_color: u32,  // 0 = default amber 0xFF_FFE040
    /// Labels/buttons: never word-wrap, even if narrower than the text.
    /// For single-line titles/headers that must stay on one line — matches
    /// Patchouli, where such text overflows its box horizontally rather than
    /// wrapping onto a second line.
    pub no_wrap: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            w: 0.0, h: 0.0, min_w: 4.0, min_h: 4.0, flex: 0.0, dock: Dock::None, gap: 2.0,
            pad: [0.0; 4], margin: [0.0; 4], bg: 0, color: 0xFF_CCCCAA,
            align: Align::Start, font_scale: 1.0, text_shadow: true,
            focus_style: FocusStyle::default(), focus_color: 0, no_wrap: false,
        }
    }
}

// ── Builder API ───────────────────────────────────────────────────────────────

impl Widget {
    fn new(kind: WidgetKind) -> Self {
        let flex_dir = if let WidgetKind::Panel(dir) = &kind { *dir } else { FlexDir::Column };
        Self { kind, id: None, style: Style::default(), flex_dir,
               children: Vec::new(), on_click: None, enabled: true, focused: false }
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
    pub fn shadow(mut self, v: bool) -> Self { self.style.text_shadow = v; self }
    pub fn no_wrap(mut self) -> Self { self.style.no_wrap = true; self }
    pub fn focus_style(mut self, v: FocusStyle) -> Self { self.style.focus_style = v; self }
    pub fn focus_color(mut self, v: u32) -> Self { self.style.focus_color = v; self }
    pub fn dock(mut self, v: Dock) -> Self { self.style.dock = v; self }
    pub fn enabled(mut self, v: bool) -> Self { self.enabled = v; self }
    pub fn focused(mut self, v: bool) -> Self { self.focused = v; self }
    pub fn padding(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        self.style.pad = [top, right, bottom, left]; self
    }
    pub fn margin(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        self.style.margin = [top, right, bottom, left]; self
    }
}

// ── Widget constructors ───────────────────────────────────────────────────────

pub fn panel(dir: FlexDir) -> Widget { Widget::new(WidgetKind::Panel(dir)) }
pub fn label(text: impl Into<String>) -> Widget { Widget::new(WidgetKind::Label(text.into())) }
pub fn button(text: impl Into<String>) -> Widget { Widget::new(WidgetKind::Button(text.into())) }
pub fn item_slot(item_id: impl Into<String>) -> Widget { Widget::new(WidgetKind::ItemSlot(item_id.into())) }
pub fn mc_image(id: impl Into<String>, img_w: f32, img_h: f32) -> Widget {
    Widget::new(WidgetKind::McImage { id: id.into(), img_w, img_h })
        .w(img_w).h(img_h)
}
pub fn spacer() -> Widget { Widget::new(WidgetKind::Spacer) }
