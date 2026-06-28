//! GPU rendering for the UI tree via `yog-gfx` draw2d.
//! Only valid inside `on_hud_render`.

use crate::layout::LayoutNode;
use crate::text;
use crate::widget::{FocusStyle, Widget, WidgetKind};
use yog_gfx::draw2d::Draw2D;

/// Recursively render a widget tree starting from the computed layout.
pub fn render_node(d2d: &Draw2D, widget: &Widget, node: &LayoutNode) {
    let r = &node.rect;
    let s = &widget.style;

    // Background
    if s.bg != 0 {
        d2d.rect(r.x, r.y, r.x + r.w, r.y + r.h, s.bg);
    }
    // Focus indicator — style controlled per-widget.
    if node.focused {
        let fc = if s.focus_color != 0 { s.focus_color } else { 0xFF_FFE040 };
        match s.focus_style {
            FocusStyle::Outline => {
                d2d.rect(r.x,             r.y,             r.x + r.w, r.y + 1.0,     fc);
                d2d.rect(r.x,             r.y + r.h - 1.0, r.x + r.w, r.y + r.h,     fc);
                d2d.rect(r.x,             r.y,             r.x + 1.0, r.y + r.h,     fc);
                d2d.rect(r.x + r.w - 1.0, r.y,            r.x + r.w, r.y + r.h,     fc);
            }
            FocusStyle::Fill => {
                d2d.rect(r.x, r.y, r.x + r.w, r.y + r.h, fc);
            }
            FocusStyle::None => {}
        }
    }

    match &widget.kind {
        WidgetKind::Panel(_) => {
            // Panel just draws children
        }
        WidgetKind::Label(t) => {
            let avail_w = (r.w - s.pad[1] - s.pad[3] - 2.0).max(0.0);
            let tx = r.x + s.pad[3] + 2.0;
            let lines = text::wrap_text(t, avail_w, s.font_scale);
            let line_h = text::LINE_H * s.font_scale + text::LINE_GAP;
            let total_h = lines.len() as f32 * line_h - text::LINE_GAP;
            let content_h = r.h - s.pad[0] - s.pad[2];
            let start_y = r.y + s.pad[0] + ((content_h - total_h) / 2.0).max(0.0);
            for (i, line) in lines.iter().enumerate() {
                d2d.text(line, tx, start_y + i as f32 * line_h, s.color, true);
            }
        }
        WidgetKind::Button(t) => {
            let avail_w = (r.w - s.pad[1] - s.pad[3] - 2.0).max(0.0);
            let lines = text::wrap_text(t, avail_w, s.font_scale);
            let line_h = text::LINE_H * s.font_scale + text::LINE_GAP;
            let total_h = lines.len() as f32 * line_h - text::LINE_GAP;
            let content_h = r.h - s.pad[0] - s.pad[2];
            let content_w = r.w - s.pad[1] - s.pad[3];
            let start_y = r.y + s.pad[0] + ((content_h - total_h) / 2.0).max(0.0);
            for (i, line) in lines.iter().enumerate() {
                // Center each line horizontally inside the button.
                let line_w = line.len() as f32 * text::CHAR_W * s.font_scale;
                let tx = r.x + s.pad[3] + ((content_w - line_w) / 2.0).max(0.0);
                d2d.text(line, tx, start_y + i as f32 * line_h, s.color, true);
            }
        }
        WidgetKind::ItemSlot(item_id) => {
            let ix = r.x + s.pad[3]; let iy = r.y + s.pad[0];
            let sz = 18.0;
            d2d.rect(ix, iy, ix + sz, iy + sz, 0xFF_444444);
            d2d.rect(ix+1.0, iy+1.0, ix+sz-1.0, iy+sz-1.0, 0xFF_888888);
            if let Some((ns, name)) = item_id.split_once(':') {
                d2d.mc_texture(&format!("{ns}:textures/item/{name}.png"),
                    ix+2.0, iy+2.0, 0.0, 0.0, 14.0, 14.0, 16.0, 16.0);
            }
        }
        WidgetKind::McImage { id, img_w, img_h } => {
            d2d.mc_texture(id, r.x + s.pad[3], r.y + s.pad[0],
                0.0, 0.0, *img_w, *img_h, *img_w, *img_h);
        }
        WidgetKind::Spacer => {} // invisible
    }

    // Render children
    for (i, child_widget) in widget.children.iter().enumerate() {
        if let Some(child_node) = node.children.get(i) {
            render_node(d2d, child_widget, child_node);
        }
    }
}
